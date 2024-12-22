use bytes::BytesMut;
use tokio::{
    io::AsyncReadExt as _,
    net::unix::pipe::Receiver,
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
    pub fn spawn(
        pipe_reader: Receiver,
        capacity: Option<usize>,
    ) -> Result<MessageReaderHandle, MessageReaderError> {
        let (mut reader, sender) = Self::create(pipe_reader, capacity)?;
        tokio::spawn(async move { reader.run().await });
        Ok(MessageReaderHandle::new(sender))
    }

    fn create(
        pipe_reader: Receiver,
        capacity: Option<usize>,
    ) -> Result<(Self, mpsc::Sender<MessageReaderCommand>), MessageReaderError> {
        let capacity = capacity.unwrap_or(16);
        if Self::is_capacity_invalid(capacity) {
            return Err(MessageReaderError::InvalidChannelCapacity(capacity));
        }
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

#[derive(Debug)]
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
}

#[cfg(test)]
mod tests {
    use std::time::Duration;
    use test_utils::TestPipe;
    use tokio::{io::AsyncWriteExt, net::unix::pipe};

    use super::*;

    #[tokio::test]
    async fn should_read_messages_from_pipe() {
        let (mut sender, receiver) = pipe::pipe().unwrap();
        let reader = MessageReader::spawn(receiver, None).unwrap();

        let writer_handle = tokio::spawn(async move {
            sender.write_all(b"Message 1\n").await.unwrap();
            sender.write_all(b"Message 2\n").await.unwrap();
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
    async fn should_read_messages_from_named_pipe() {
        let pipe = TestPipe::new();
        let reader = MessageReader::spawn(pipe.reader(), None).unwrap();

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
        let reader = MessageReader::spawn(pipe.reader(), None).unwrap();

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
        let reader = MessageReader::spawn(pipe.reader(), None).unwrap();
        reader.abort().await;
        assert!(reader.subscribe().await.is_err());
    }

    #[tokio::test]
    async fn should_return_err_when_capacity_is_invalid() {
        let pipe = TestPipe::new();

        let result = MessageReader::spawn(pipe.reader(), Some(0));
        assert!(matches!(
            result,
            Err(MessageReaderError::InvalidChannelCapacity(_))
        ));

        let result = MessageReader::spawn(pipe.reader(), Some(usize::MAX / 2 + 1));
        assert!(matches!(
            result,
            Err(MessageReaderError::InvalidChannelCapacity(_))
        ));
    }
}
