use std::{collections::HashMap, io, path::PathBuf};

use tokio::sync::{
    broadcast::{self},
    mpsc, oneshot,
};
use tokio_stream::{
    wrappers::{errors::BroadcastStreamRecvError, BroadcastStream},
    Stream, StreamExt,
};

use crate::working_dir::WorkingDir;

use super::{
    model::{Process, ProcessId},
    reader::{MessageReader, MessageReaderError, MessageReaderHandle},
    spawner::SpawnerError,
    Cmd,
};
use super::{
    spawner::ProcessSpawner,
    writer::{MessageWriter, MessageWriterHandle},
};

#[derive(Debug)]
enum ProcessManagerMessage {
    SpawnProcess {
        cmd: Cmd,
        responder: oneshot::Sender<Result<ProcessId, SpawnProcessError>>,
    },
    SubscribeMessageStream {
        id: ProcessId,
        responder: oneshot::Sender<Result<broadcast::Receiver<Vec<u8>>, ReadMessageError>>,
    },
    WriteMessage {
        id: ProcessId,
        data: Vec<u8>,
        responder: oneshot::Sender<Result<(), WriteMessageError>>,
    },
    KillProcess {
        id: ProcessId,
        responder: oneshot::Sender<()>,
    },
}

pub struct ProcessManager {
    process_spawner: ProcessSpawner,
    processes: HashMap<ProcessId, Process>,
    message_writers: HashMap<ProcessId, MessageWriterHandle>,
    message_readers: HashMap<ProcessId, MessageReaderHandle>,
    receiver: mpsc::Receiver<ProcessManagerMessage>,
}

impl ProcessManager {
    pub fn new(working_dir: PathBuf) -> ProcessManagerHandle {
        let (sender, receiver) = mpsc::channel(8);
        let mut manager = Self {
            process_spawner: ProcessSpawner::new(WorkingDir::new(working_dir)),
            receiver,
            processes: HashMap::new(),
            message_writers: HashMap::new(),
            message_readers: HashMap::new(),
        };
        tokio::spawn(async move { manager.run().await });
        ProcessManagerHandle::new(sender)
    }

    async fn run(&mut self) {
        while let Some(msg) = self.receiver.recv().await {
            self.handle_message(msg).await;
        }
    }

    async fn handle_message(&mut self, msg: ProcessManagerMessage) {
        match msg {
            ProcessManagerMessage::SpawnProcess { cmd, responder } => {
                let result = self.spawn_process(cmd).await;
                let _ = responder.send(result);
            }
            ProcessManagerMessage::KillProcess { id, responder } => {
                self.kill_process(id).await;
                let _ = responder.send(());
            }
            ProcessManagerMessage::SubscribeMessageStream { id, responder } => {
                let result = self.subscribe_to_message_stream(id).await;
                let _ = responder.send(result);
            }
            ProcessManagerMessage::WriteMessage {
                id,
                data,
                responder,
            } => {
                let result = self.write_message(id, data).await;
                let _ = responder.send(result);
            }
        }
    }

    async fn spawn_process(&mut self, cmd: Cmd) -> Result<ProcessId, SpawnProcessError> {
        let id = ProcessId::random();
        let options = cmd.options.clone(); // TODO: moze te spawnanie tych readerów, writerów przenniesc do spanwer'a
        let process = self.process_spawner.spawn(&id, cmd)?;

        if let Some(ref input_pipe) = process.input_pipe {
            let writer = MessageWriter::new(input_pipe)?;
            self.message_writers.insert(id.clone(), writer);
        }

        if let Some(ref output_pipe) = process.output_pipe {
            let reader = MessageReader::new(output_pipe, options.output_buffer_capacity)?;
            self.message_readers.insert(id.clone(), reader);
        }

        self.processes.insert(id.clone(), process);
        Ok(id)
    }

    async fn kill_process(&mut self, id: ProcessId) {
        if let Some(reader) = self.message_readers.remove(&id) {
            reader.abort().await;
        }
        if let Some(writer) = self.message_writers.remove(&id) {
            writer.abort().await;
        }
        self.processes.remove(&id); // kill_on_drop() is used to kill child process
    }

    async fn subscribe_to_message_stream(
        &mut self,
        id: ProcessId,
    ) -> Result<broadcast::Receiver<Vec<u8>>, ReadMessageError> {
        let reader = self
            .message_readers
            .get(&id)
            .ok_or(ReadMessageError::ProcessNotFound(id))?;
        let receiver = reader
            .subscribe()
            .await
            .map_err(|_| ReadMessageError::MessageReaderKilled)?;
        Ok(receiver)
    }

    async fn write_message(
        &mut self,
        id: ProcessId,
        data: Vec<u8>,
    ) -> Result<(), WriteMessageError> {
        let writer = self
            .message_writers
            .get(&id)
            .ok_or(WriteMessageError::ProcessNotFound(id))?;
        writer.write(data).await.map_err(Into::into)
    }
}

#[derive(Clone)]
pub struct ProcessManagerHandle {
    sender: mpsc::Sender<ProcessManagerMessage>,
}

impl ProcessManagerHandle {
    fn new(sender: mpsc::Sender<ProcessManagerMessage>) -> Self {
        Self { sender }
    }

    pub async fn spawn(&self, cmd: Cmd) -> Result<ProcessId, SpawnProcessError> {
        let (responder, receiver) = oneshot::channel();
        let msg = ProcessManagerMessage::SpawnProcess { cmd, responder };
        let _ = self.sender.send(msg).await;
        let process_id = receiver.await??;
        Ok(process_id)
    }

    pub async fn subscribe_message_stream_raw(
        &self,
        id: ProcessId,
    ) -> Result<impl Stream<Item = Result<Vec<u8>, ReceiveMessageError>>, ReadMessageError> {
        let (responder, receiver) = oneshot::channel();
        let msg = ProcessManagerMessage::SubscribeMessageStream { id, responder };
        let _ = self.sender.send(msg).await;
        let message_receiver = receiver.await??;
        let stream = BroadcastStream::new(message_receiver).map(|value| value.map_err(Into::into));
        Ok(stream)
    }

    pub async fn subscribe_message_stream(
        &self,
        id: ProcessId,
    ) -> Result<impl Stream<Item = Vec<u8>>, ReadMessageError> {
        Ok(self
            .subscribe_message_stream_raw(id)
            .await?
            .filter_map(Result::ok))
    }

    pub async fn write_message<T: TryInto<Vec<u8>>>(
        &self,
        id: ProcessId,
        data: T,
    ) -> Result<(), WriteMessageError> {
        let bytes = data
            .try_into()
            .map_err(|_| WriteMessageError::CannotSerializeMessage)?;

        if let Some(data) = Self::into_bytes_with_eol_char(bytes) {
            let (responder, receiver) = oneshot::channel();
            let msg = ProcessManagerMessage::WriteMessage {
                id,
                data,
                responder,
            };
            let _ = self.sender.send(msg).await;
            receiver.await??;
        }
        Ok(())
    }

    fn into_bytes_with_eol_char(mut bytes: Vec<u8>) -> Option<Vec<u8>> {
        if let Some(last_char) = bytes.last() {
            if *last_char != b'\n' {
                bytes.push(b'\n');
            }
            Some(bytes)
        } else {
            None
        }
    }

    pub async fn kill(&self, id: ProcessId) -> Result<(), KillProcessError> {
        let (responder, receiver) = oneshot::channel();
        let msg = ProcessManagerMessage::KillProcess { id, responder };
        let _ = self.sender.send(msg).await;
        receiver.await.map_err(Into::into)
    }
}

#[derive(thiserror::Error, Debug)]
pub enum ReceiveMessageError {
    #[error("Lost {0} number of messages due to buffer capacity overflow")]
    LostMessages(u64),
}

impl From<BroadcastStreamRecvError> for ReceiveMessageError {
    fn from(err: BroadcastStreamRecvError) -> Self {
        match err {
            BroadcastStreamRecvError::Lagged(size) => ReceiveMessageError::LostMessages(size),
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum SpawnProcessError {
    #[error("Cannot communicate with spawned process manager")]
    ManagerCommunicationError(#[from] oneshot::error::RecvError),
    #[error("Cannot create named pipe at path: {0}. Error code: {1}")]
    CannotCreateNamedPipe(PathBuf, String),
    #[error("Cannot spawn process: {0}")]
    CannotSpawnProcess(#[from] io::Error),
    #[error("Cannot spawn process: {0}")]
    CannotCreateProcessWorkingDir(io::Error),
    #[error("Invalid output buffer capacity: {0}")]
    InvalidOutputBufferCapacity(usize),
    #[error("Unexpected IO error: {0}")]
    UnexpectedIoError(io::Error),
}

impl From<SpawnerError> for SpawnProcessError {
    fn from(value: SpawnerError) -> Self {
        match value {
            SpawnerError::CannotCreateNamedPipe(pipe_path, err) => {
                SpawnProcessError::CannotCreateNamedPipe(pipe_path, err.to_string())
            }
            SpawnerError::CannotSpawnProcess(err) => SpawnProcessError::CannotSpawnProcess(err),
            SpawnerError::CannotCreateProcessWorkingDir(err) => {
                SpawnProcessError::CannotCreateProcessWorkingDir(err)
            }
        }
    }
}

impl From<MessageReaderError> for SpawnProcessError {
    fn from(value: MessageReaderError) -> Self {
        match value {
            MessageReaderError::InvalidChannelCapacity(value) => {
                SpawnProcessError::InvalidOutputBufferCapacity(value)
            }
            MessageReaderError::UnexpectedIoError(err) => SpawnProcessError::UnexpectedIoError(err),
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum ReadMessageError {
    #[error("Cannot communicate with spawned process manager")]
    ManagerCommunicationError(#[from] oneshot::error::RecvError),
    #[error("Process with id: {0} was not found")]
    ProcessNotFound(ProcessId),
    #[error("Message reader process has been killed")]
    MessageReaderKilled,
}

#[derive(thiserror::Error, Debug)]
pub enum WriteMessageError {
    #[error("Cannot serialize message to bytes")]
    CannotSerializeMessage,
    #[error("Cannot communicate with spawned process manager")]
    ManagerCommunicationError(#[from] oneshot::error::RecvError),
    #[error("Error occurred when writing message to process: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Process with id: {0} was not found")]
    ProcessNotFound(ProcessId),
}

#[derive(thiserror::Error, Debug)]
pub enum KillProcessError {
    #[error("Cannot communicate with spawned process manager")]
    ManagerCommunicationError(#[from] oneshot::error::RecvError),
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;
    use tokio_stream::StreamExt;

    use crate::{Cmd, CmdBuilder};

    use super::ProcessManager;

    #[tokio::test]
    async fn should_spawn_process() {
        let working_dir = tempdir().unwrap();
        let handle = ProcessManager::new(working_dir.into_path());
        let result = handle.spawn(cat_cmd()).await;
        assert!(result.is_ok());
    }

    #[ignore] // TODO
    #[tokio::test]
    async fn should_get_message_stream() {
        let working_dir = tempdir().unwrap();
        let handle = ProcessManager::new(working_dir.into_path());
        let process_id = handle.spawn(echo_cmd()).await.unwrap();

        let mut stream = handle.subscribe_message_stream(process_id).await.unwrap();
        assert_eq!(b"msg1", stream.next().await.unwrap().as_slice());
    }

    // TODO: move to utils
    fn cat_cmd() -> Cmd {
        CmdBuilder::default().cmd("cat".into()).build().unwrap()
    }

    fn echo_cmd() -> Cmd {
        CmdBuilder::default()
            .cmd("echo".into())
            .args(vec!["msg1\n".into(), ">".into(), "$OUTPUT_PIPE".into()].into())
            .build()
            .unwrap()
    }

    // TODO: write more tests
}
