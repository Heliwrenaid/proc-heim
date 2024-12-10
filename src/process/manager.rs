use std::{collections::HashMap, io, path::PathBuf};

use tokio::sync::{
    broadcast::{self},
    mpsc, oneshot,
};
use tokio_stream::{
    wrappers::{errors::BroadcastStreamRecvError, BroadcastStream},
    Stream, StreamExt,
};

use crate::{working_dir::WorkingDir, ProcessData};

use super::{
    log_reader::{LogReaderError, LogsQuery, LogsQueryType},
    spawner::ProcessSpawner,
};
use super::{
    model::{Process, ProcessId},
    reader::MessageReaderError,
    spawner::SpawnerError,
    Cmd,
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
        responder: oneshot::Sender<Result<(), KillProcessError>>,
    },
    GetLogs {
        id: ProcessId,
        logs_query_type: LogsQueryType,
        query: LogsQuery,
        responder: oneshot::Sender<Result<Vec<String>, GetLogsError>>,
    },
    GetProcessData {
        id: ProcessId,
        responder: oneshot::Sender<Result<ProcessData, GetProcessDataError>>,
    },
}

pub struct ProcessManager {
    process_spawner: ProcessSpawner,
    processes: HashMap<ProcessId, Process>,
    receiver: mpsc::Receiver<ProcessManagerMessage>,
}

impl ProcessManager {
    pub fn new(working_dir: PathBuf) -> ProcessManagerHandle {
        let (sender, receiver) = mpsc::channel(8);
        let mut manager = Self {
            process_spawner: ProcessSpawner::new(WorkingDir::new(working_dir)),
            receiver,
            processes: HashMap::new(),
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
                let result = self.kill_process(id).await;
                let _ = responder.send(result);
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
            ProcessManagerMessage::GetLogs {
                id,
                logs_query_type,
                query,
                responder,
            } => {
                let result = self.get_logs(id, logs_query_type, query).await;
                let _ = responder.send(result);
            }
            ProcessManagerMessage::GetProcessData { id, responder } => {
                let result = self.get_process_data(id);
                let _ = responder.send(result);
            }
        }
    }

    async fn spawn_process(&mut self, cmd: Cmd) -> Result<ProcessId, SpawnProcessError> {
        let id = ProcessId::random();
        let process = self.process_spawner.spawn(&id, cmd)?;
        self.processes.insert(id.clone(), process);
        Ok(id)
    }

    async fn kill_process(&mut self, id: ProcessId) -> Result<(), KillProcessError> {
        let process = self
            .processes
            .get_mut(&id)
            .ok_or(KillProcessError::ProcessNotFound(id))?;
        if let Some(reader) = process.message_reader.take() {
            reader.abort().await;
        }
        if let Some(writer) = process.message_writer.take() {
            writer.abort().await;
        }
        if let Some(log_reader) = process.log_reader.take() {
            log_reader.abort().await;
        }
        self.processes.remove(&id); // kill_on_drop() is used to kill child process
        Ok(())
    }

    async fn subscribe_to_message_stream(
        &mut self,
        id: ProcessId,
    ) -> Result<broadcast::Receiver<Vec<u8>>, ReadMessageError> {
        let reader = self
            .processes
            .get(&id)
            .ok_or(ReadMessageError::ProcessNotFound(id))?
            .message_reader
            .as_ref()
            .ok_or(ReadMessageError::MessageReaderNotFound(id))?;

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
        self.processes
            .get(&id)
            .ok_or(WriteMessageError::ProcessNotFound(id))?
            .message_writer
            .as_ref()
            .ok_or(WriteMessageError::MessageWriterNotFound(id))?
            .write(data)
            .await
            .map_err(Into::into)
    }

    async fn get_logs(
        &mut self,
        id: ProcessId,
        logs_query_type: LogsQueryType,
        query: LogsQuery,
    ) -> Result<Vec<String>, GetLogsError> {
        self.processes
            .get(&id)
            .ok_or(GetLogsError::ProcessNotFound(id))?
            .log_reader
            .as_ref()
            .ok_or(GetLogsError::LogReaderNotFound(id))?
            .read_logs(logs_query_type, query)
            .await
            .map_err(Into::into)
    }

    fn get_process_data(&mut self, id: ProcessId) -> Result<ProcessData, GetProcessDataError> {
        let process = self
            .processes
            .get_mut(&id)
            .ok_or(GetProcessDataError::ProcessNotFound(id))?;
        let pid = process.child.id();
        let exit_status = process.child.try_wait()?;
        Ok(ProcessData::new(pid, exit_status))
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

    pub async fn subscribe_message_bytes_stream(
        &self,
        id: ProcessId,
    ) -> Result<impl Stream<Item = Result<Vec<u8>, ReceiveMessageBytesError>>, ReadMessageError>
    {
        let (responder, receiver) = oneshot::channel();
        let msg = ProcessManagerMessage::SubscribeMessageStream { id, responder };
        let _ = self.sender.send(msg).await;
        let message_receiver = receiver.await??;
        let stream = BroadcastStream::new(message_receiver).map(|v| v.map_err(Into::into));
        Ok(stream)
    }

    pub async fn subscribe_message_stream<T: TryFrom<Vec<u8>>>(
        &self,
        id: ProcessId,
    ) -> Result<impl Stream<Item = Result<T, ReceiveMessageError>>, ReadMessageError> {
        Ok(self
            .subscribe_message_bytes_stream(id)
            .await?
            .map(Self::to_message))
    }

    fn to_message<T: TryFrom<Vec<u8>>>(
        bytes: Result<Vec<u8>, ReceiveMessageBytesError>,
    ) -> Result<T, ReceiveMessageError> {
        let bytes = bytes?;
        T::try_from(bytes).map_err(|_| {
            ReceiveMessageError::CannotDeserializeMessage(
                "Cannot deserialize data from raw bytes using TryFrom trait".into(),
            )
        })
    }

    pub async fn write_message<T: TryInto<Vec<u8>>>(
        &self,
        id: ProcessId,
        data: T,
    ) -> Result<(), WriteMessageError> {
        let bytes = data.try_into().map_err(|_| {
            WriteMessageError::CannotSerializeMessage(
                "Cannot serialize message to bytes using TryInto trait".into(),
            )
        })?;

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
        receiver.await??;
        Ok(())
    }

    pub async fn get_logs_stdout(
        &self,
        id: ProcessId,
        query: LogsQuery,
    ) -> Result<Vec<String>, GetLogsError> {
        self.get_logs(id, LogsQueryType::Stdout, query).await
    }

    pub async fn get_logs_stderr(
        &self,
        id: ProcessId,
        query: LogsQuery,
    ) -> Result<Vec<String>, GetLogsError> {
        self.get_logs(id, LogsQueryType::Stderr, query).await
    }

    async fn get_logs(
        &self,
        id: ProcessId,
        logs_query_type: LogsQueryType,
        query: LogsQuery,
    ) -> Result<Vec<String>, GetLogsError> {
        let (responder, receiver) = oneshot::channel();
        let msg = ProcessManagerMessage::GetLogs {
            id,
            logs_query_type,
            query,
            responder,
        };
        let _ = self.sender.send(msg).await;
        let logs = receiver.await??;
        Ok(logs)
    }

    pub async fn get_process_data(
        &self,
        id: ProcessId,
    ) -> Result<ProcessData, GetProcessDataError> {
        let (responder, receiver) = oneshot::channel();
        let msg = ProcessManagerMessage::GetProcessData { id, responder };
        let _ = self.sender.send(msg).await;
        let data = receiver.await??;
        Ok(data)
    }
}

#[cfg(any(feature = "json", feature = "message-pack"))]
use super::serde::{DataFormat, SerdeUtil};

#[cfg(any(feature = "json", feature = "message-pack"))]
impl ProcessManagerHandle {
    pub async fn subscribe_message_stream_with_format<T: serde::de::DeserializeOwned>(
        &self,
        id: ProcessId,
        format: DataFormat,
    ) -> Result<impl Stream<Item = Result<T, ReceiveMessageError>>, ReadMessageError> {
        Ok(self
            .subscribe_message_bytes_stream(id)
            .await?
            .map(move |bytes| Self::deserialize_message(bytes, &format)))
    }

    fn deserialize_message<T: serde::de::DeserializeOwned>(
        bytes: Result<Vec<u8>, ReceiveMessageBytesError>,
        format: &DataFormat,
    ) -> Result<T, ReceiveMessageError> {
        SerdeUtil::deserialize(&bytes?, format)
            .map_err(|err| ReceiveMessageError::CannotDeserializeMessage(err.to_string()))
    }

    pub async fn write_messages_with_format<T: serde::Serialize>(
        &self,
        id: ProcessId,
        data: T,
        format: DataFormat,
    ) -> Result<(), WriteMessageError> {
        let bytes = SerdeUtil::serialize(&data, &format)
            .map_err(|err| WriteMessageError::CannotSerializeMessage(err.to_string()))?;
        self.write_message(id, bytes).await
    }
}

#[derive(thiserror::Error, Debug)]
pub enum ReceiveMessageBytesError {
    #[error("Lost {0} number of messages due to buffer capacity overflow")]
    LostMessages(u64),
}

#[derive(thiserror::Error, Debug)]
pub enum ReceiveMessageError {
    #[error("Lost {0} number of messages due to buffer capacity overflow")]
    LostMessages(u64),
    #[error("{0}")]
    CannotDeserializeMessage(String),
}

impl From<BroadcastStreamRecvError> for ReceiveMessageBytesError {
    fn from(err: BroadcastStreamRecvError) -> Self {
        match err {
            BroadcastStreamRecvError::Lagged(size) => ReceiveMessageBytesError::LostMessages(size),
        }
    }
}

impl From<ReceiveMessageBytesError> for ReceiveMessageError {
    fn from(value: ReceiveMessageBytesError) -> Self {
        match value {
            ReceiveMessageBytesError::LostMessages(number) => {
                ReceiveMessageError::LostMessages(number)
            }
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
            SpawnerError::InvalidOutputBufferCapacity(value) => {
                SpawnProcessError::InvalidOutputBufferCapacity(value)
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
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum ReadMessageError {
    #[error("Process with id: {0} was not found")]
    ProcessNotFound(ProcessId),
    #[error("Cannot communicate with spawned process manager")]
    ManagerCommunicationError(#[from] oneshot::error::RecvError),
    #[error("Message reader for process with id: {0} was not found")]
    MessageReaderNotFound(ProcessId),
    #[error("Message reader process has been killed")]
    MessageReaderKilled,
}

#[derive(thiserror::Error, Debug)]
pub enum WriteMessageError {
    #[error("Process with id: {0} was not found")]
    ProcessNotFound(ProcessId),
    #[error("{0}")]
    CannotSerializeMessage(String),
    #[error("Cannot communicate with spawned process manager")]
    ManagerCommunicationError(#[from] oneshot::error::RecvError),
    #[error("Error occurred when writing message to process: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Message writer for process with id: {0} was not found")]
    MessageWriterNotFound(ProcessId),
}

#[derive(thiserror::Error, Debug)]
pub enum KillProcessError {
    #[error("Process with id: {0} was not found")]
    ProcessNotFound(ProcessId),
    #[error("Cannot communicate with spawned process manager")]
    ManagerCommunicationError(#[from] oneshot::error::RecvError),
}

#[derive(thiserror::Error, Debug)]
pub enum GetLogsError {
    #[error("Process with id: {0} was not found")]
    ProcessNotFound(ProcessId),
    #[error("Cannot communicate with spawned process manager")]
    ManagerCommunicationError(#[from] oneshot::error::RecvError),
    #[error("Logging type: {0} was not configured for process")]
    LoggingTypeWasNotConfigured(String),
    #[error("Log read for process with id: {0} was not found")]
    LogReaderNotFound(ProcessId),
    #[error(transparent)]
    UnExpectedIoError(#[from] io::Error),
}

impl From<LogReaderError> for GetLogsError {
    fn from(err: LogReaderError) -> Self {
        match err {
            LogReaderError::LogTypeWasNotConfigured(log_type) => {
                Self::LoggingTypeWasNotConfigured(log_type.to_string())
            }
            LogReaderError::UnExpectedIoError(err) => Self::UnExpectedIoError(err),
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum GetProcessDataError {
    #[error("Process with id: {0} was not found")]
    ProcessNotFound(ProcessId),
    #[error("Cannot communicate with spawned process manager")]
    ManagerCommunicationError(#[from] oneshot::error::RecvError),
    #[error(transparent)]
    UnExpectedIoError(#[from] io::Error),
}
