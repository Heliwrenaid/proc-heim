use std::{io, path::PathBuf};

use bytes::BytesMut;
use tokio::{
    io::AsyncReadExt as _,
    net::unix::{self, pipe::Receiver},
    select,
    sync::{
        broadcast::{self},
        mpsc::{self},
        oneshot,
    },
};

enum MessageReaderCommand {
    Subscribe {
        responder: oneshot::Sender<broadcast::Receiver<Vec<u8>>>,
    },
    Abort,
}

pub struct MessageReader {
    pipe_reader: Receiver,
    subscription_receiver: mpsc::Receiver<MessageReaderCommand>,
    message_broadcaster: broadcast::Sender<Vec<u8>>,
    message_receiver: Option<broadcast::Receiver<Vec<u8>>>,
    abort: bool,
}

impl MessageReader {
    pub fn new(
        pipe_path: &PathBuf,
        capacity: Option<usize>,
    ) -> Result<MessageReaderHandle, MessageReaderError> {
        let (mut reader, sender) = Self::create(pipe_path, capacity)?;
        tokio::spawn(async move { reader.run().await });
        Ok(MessageReaderHandle::new(sender))
    }

    fn create(
        pipe_path: &PathBuf,
        capacity: Option<usize>,
    ) -> Result<(Self, mpsc::Sender<MessageReaderCommand>), MessageReaderError> {
        let capacity = capacity.unwrap_or(16);
        if Self::is_capacity_invalid(capacity) {
            return Err(MessageReaderError::InvalidChannelCapacity(capacity));
        }

        let pipe_reader = unix::pipe::OpenOptions::new()
            .read_write(true)
            .open_receiver(pipe_path)?;
        let (sender, subscription_receiver) = mpsc::channel(32);

        let (message_broadcaster, message_receiver) = broadcast::channel(capacity);
        let reader = MessageReader {
            pipe_reader,
            subscription_receiver,
            message_broadcaster,
            message_receiver: Some(message_receiver),
            abort: false,
        };
        Ok((reader, sender))
    }

    fn is_capacity_invalid(capacity: usize) -> bool {
        capacity == 0 || capacity > usize::MAX / 2
    }

    async fn run(&mut self) {
        loop {
            select! {
                Some(msg) = self.subscription_receiver.recv() => {
                    self.handle_message(msg).await;
                },
                _ = self.pipe_reader.readable() => {
                    self.read_message().await;
                }
            }
            if self.abort {
                break;
            }
        }
    }

    async fn handle_message(&mut self, msg: MessageReaderCommand) {
        match msg {
            MessageReaderCommand::Subscribe { responder } => {
                let receiver = if self.message_receiver.is_some() {
                    self.message_receiver.take().unwrap()
                } else {
                    self.message_broadcaster.subscribe()
                };
                let _ = responder.send(receiver);
            }
            MessageReaderCommand::Abort => {
                self.abort = true;
            }
        }
    }

    async fn read_message(&mut self) {
        let mut buf = BytesMut::with_capacity(4096);
        if self.pipe_reader.read_buf(&mut buf).await.is_ok() {
            buf.split_inclusive(|byte| *byte == b'\n')
                .map(|msg| msg.strip_suffix(b"\n").unwrap_or(msg))
                .for_each(|msg| {
                    let _ = self.message_broadcaster.send(msg.into());
                });
        }
    }
}

pub struct MessageReaderHandle {
    sender: mpsc::Sender<MessageReaderCommand>,
}

impl MessageReaderHandle {
    fn new(sender: mpsc::Sender<MessageReaderCommand>) -> Self {
        Self { sender }
    }

    pub async fn subscribe(
        &self,
    ) -> Result<broadcast::Receiver<Vec<u8>>, oneshot::error::RecvError> {
        let (responder, receiver) = oneshot::channel();
        let _ = self
            .sender
            .send(MessageReaderCommand::Subscribe { responder })
            .await;
        receiver.await
    }

    pub async fn abort(&self) {
        let _ = self.sender.send(MessageReaderCommand::Abort).await;
    }
}

#[derive(thiserror::Error, Debug)]
pub enum MessageReaderError {
    #[error("Invalid channel capacity: {0}")]
    InvalidChannelCapacity(usize),
    #[error(transparent)]
    UnexpectedIoError(#[from] io::Error),
}

#[cfg(test)]
mod tests {
    use std::{fs::File, time::Duration};

    use tempfile::tempdir;
    use test_utils::TestPipe;
    use tokio::io::AsyncWriteExt;

    use super::*;

    #[tokio::test]
    async fn should_read_messages_from_named_pipe() {
        let pipe = TestPipe::new();
        let reader = MessageReader::new(&pipe.pipe_path, None).unwrap();

        let mut writer = pipe.writer();
        let writer_handle = tokio::spawn(async move {
            writer.write_all(b"Message 1\n").await.unwrap();
            writer.write_all(b"Message 2\n").await.unwrap();
        });

        let mut receiver = reader.subscribe().await.unwrap();

        let msg = receiver.recv().await.unwrap();
        assert_eq!(b"Message 1", &msg[..]);

        let msg = receiver.recv().await.unwrap();
        assert_eq!(b"Message 2", &msg[..]);

        writer_handle.await.unwrap();
        assert!(receiver.try_recv().is_err());
    }

    #[tokio::test]
    async fn should_subscribe_multiple_times() {
        let pipe = TestPipe::new();
        let reader = MessageReader::new(&pipe.pipe_path, None).unwrap();

        let mut writer = pipe.writer();
        let writer_handle = tokio::spawn(async move {
            writer.write_all(b"Message 1\n").await.unwrap();
            tokio::time::sleep(Duration::from_secs(1)).await;
            writer.write_all(b"Message 2\n").await.unwrap();
        });

        let mut receiver = reader.subscribe().await.unwrap();
        let msg = receiver.recv().await.unwrap();
        assert_eq!(b"Message 1", &msg[..]);

        let mut receiver2 = reader.subscribe().await.unwrap();
        let msg = receiver2.recv().await.unwrap();
        assert_eq!(b"Message 2", &msg[..]);

        let msg = receiver.recv().await.unwrap();
        assert_eq!(b"Message 2", &msg[..]);

        writer_handle.await.unwrap();
    }

    #[tokio::test]
    async fn should_abort_reader_process() {
        let pipe = TestPipe::new();
        let reader = MessageReader::new(&pipe.pipe_path, None).unwrap();
        reader.abort().await;
        assert!(reader.subscribe().await.is_err());
    }

    #[test]
    fn should_return_err_when_capacity_is_invalid() {
        let pipe = TestPipe::new();

        let result = MessageReader::new(&pipe.pipe_path, Some(0));
        assert!(matches!(
            result,
            Err(MessageReaderError::InvalidChannelCapacity(_))
        ));

        let result = MessageReader::new(&pipe.pipe_path, Some(usize::MAX / 2 + 1));
        assert!(matches!(
            result,
            Err(MessageReaderError::InvalidChannelCapacity(_))
        ));
    }

    #[test]
    fn should_return_err_when_file_is_not_found() {
        let invalid_file_path = "random_file_path".try_into().unwrap();
        let result = MessageReader::new(&invalid_file_path, None);
        assert!(matches!(
            result,
            Err(MessageReaderError::UnexpectedIoError(_))
        ));
    }

    #[test]
    fn should_return_err_when_file_is_not_a_pipe() {
        let tmp_dir = tempdir().unwrap();
        let file_path = tmp_dir.path().join("not_a_pipe");
        let _ = File::create(&file_path).unwrap();
        let result = MessageReader::new(&file_path, None);
        assert!(matches!(
            result,
            Err(MessageReaderError::UnexpectedIoError(_))
        ));
    }
}
