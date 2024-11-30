use std::io;

use bytes::BytesMut;
use tokio::{
    io::AsyncWriteExt as _,
    net::unix::pipe,
    sync::{mpsc, oneshot},
};

enum MessageWriterCommand {
    Write {
        data: Vec<u8>,
        responder: oneshot::Sender<io::Result<()>>,
    },
    Abort,
    HealthCheck,
}

pub struct MessageWriter {
    pipe_writer: pipe::Sender,
    receiver: mpsc::Receiver<MessageWriterCommand>,
}

impl MessageWriter {
    pub fn new(pipe_writer: pipe::Sender) -> Result<MessageWriterHandle, io::Error> {
        let (sender, receiver) = mpsc::channel(8);
        let mut manager = Self {
            pipe_writer,
            receiver,
        };
        tokio::spawn(async move { manager.run().await });
        Ok(MessageWriterHandle::new(sender))
    }

    async fn run(&mut self) {
        while let Some(command) = self.receiver.recv().await {
            match command {
                MessageWriterCommand::Write { data, responder } => {
                    let result = self.write_message(data).await;
                    let _ = responder.send(result);
                }
                MessageWriterCommand::Abort => break,
                MessageWriterCommand::HealthCheck => {}
            }
        }
    }

    async fn write_message(&mut self, data: Vec<u8>) -> io::Result<()> {
        let mut msg = BytesMut::with_capacity(data.len());
        msg.extend_from_slice(&data);
        self.pipe_writer.write_all_buf(&mut msg).await?;
        self.pipe_writer.flush().await
    }
}

#[derive(Debug)]
pub struct MessageWriterHandle {
    sender: mpsc::Sender<MessageWriterCommand>,
}

impl MessageWriterHandle {
    fn new(sender: mpsc::Sender<MessageWriterCommand>) -> Self {
        Self { sender }
    }

    pub async fn write(&self, data: Vec<u8>) -> io::Result<()> {
        let (responder, receiver) = oneshot::channel();
        let _ = self
            .sender
            .send(MessageWriterCommand::Write { data, responder })
            .await;
        match receiver.await {
            Ok(result) => result,
            Err(_) => Ok(()), // ignore error, MessageWriter is killed
        }
    }

    pub async fn abort(&self) {
        let _ = self.sender.send(MessageWriterCommand::Abort).await;
    }

    #[allow(unused)] // used only for tests
    async fn is_alive(&self) -> bool {
        self.sender
            .send(MessageWriterCommand::HealthCheck)
            .await
            .is_ok()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use test_utils::TestPipe;
    use tokio::io::AsyncReadExt as _;

    #[tokio::test]
    async fn should_write_data_to_pipe() {
        let (sender, mut receiver) = pipe::pipe().unwrap();
        let writer_handle = MessageWriter::new(sender).unwrap();
        test_writing_data(&mut receiver, &writer_handle, b"Hello world\n").await;
        test_writing_data(&mut receiver, &writer_handle, b"Just a message\n").await;
    }

    #[tokio::test]
    async fn should_write_data_to_named_pipe() {
        let pipe = TestPipe::new();
        let writer_handle = MessageWriter::new(pipe.writer()).unwrap();
        let mut reader = pipe.reader();
        test_writing_data(&mut reader, &writer_handle, b"Hello world\n").await;
        test_writing_data(&mut reader, &writer_handle, b"Next message: lorem ipsum\n").await;
    }

    async fn test_writing_data(
        receiver: &mut pipe::Receiver,
        writer_handle: &MessageWriterHandle,
        data: &[u8],
    ) {
        writer_handle.write(data.into()).await.unwrap();

        let mut buf = Vec::new();
        receiver.read_buf(&mut buf).await.unwrap();
        assert_eq!(data.to_vec(), buf);
    }

    #[tokio::test]
    async fn should_abort_writer_process() {
        let pipe = TestPipe::new();
        let writer = MessageWriter::new(pipe.writer()).unwrap();
        writer.abort().await;
        tokio::time::sleep(Duration::from_secs(1)).await; // wait, because abort() returns immediately
        assert!(!writer.is_alive().await);
    }
}
