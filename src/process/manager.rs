use std::{collections::HashMap, fmt::Debug, io, path::PathBuf, time::Duration};

use tokio::{
    sync::{broadcast, mpsc, oneshot},
    task::JoinHandle,
};
use tokio_stream::{
    wrappers::{errors::BroadcastStreamRecvError, BroadcastStream},
    Stream, StreamExt,
};

use crate::working_dir::WorkingDir;

use super::{
    log_reader::{LogReaderError, LogsQuery, LogsQueryType},
    spawner::ProcessSpawner,
    ProcessHandle, ProcessInfo, Runnable,
};
use super::{
    model::{Process, ProcessId},
    spawner::SpawnerError,
};

#[derive(Debug)]
enum ProcessManagerMessage {
    SpawnProcess {
        cmd: Box<dyn Runnable>,
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
    GetProcessInfo {
        id: ProcessId,
        responder: oneshot::Sender<Result<ProcessInfo, GetProcessInfoError>>,
    },
}

/// `ProcessManager` provides asynchronous API for spawning and managing multiple processes.
///
/// The implementation relies on the `Actor model` architecture which benefits from high concurrency and scalability and loose-coupling between actors (units doing some work).
/// `ProcessManager` is a main actor responsible for:
/// * spawning new actors (eg. for writing/reading messages to/from user-defined processes),
/// * forwarding messages between client and other actors.
///
/// All files needed to handle spawned processes are kept in user-specified directory, called further `working directory`.
/// Each process has own, unique subdirectory (`process directory`) inside `working directory`,
/// where `ProcessManager` creates files/named pipes used for communication, logging etc.
/// If the process has been killed manually, then its `process directory` is removed immediately.
///
/// Each spawned process has its own [`ProcessId`], which can be used to interact with it through [`ProcessManagerHandle`].
/// For convenience of interacting with one process, use a [`ProcessHandle`] wrapper.
pub struct ProcessManager {
    process_spawner: ProcessSpawner,
    processes: HashMap<ProcessId, Process>,
    receiver: mpsc::Receiver<ProcessManagerMessage>,
}

// TODO: should add task which remove process dirs when process exited. The the process directory can be used for a process as own temp directory.
impl ProcessManager {
    /// Spawns a new process manager task with a given `working_directory`, returning a handle associated with the manager's task.
    /// # Examples
    /// ```no_run
    /// # use proc_heim::manager::ProcessManager;
    /// # use std::path::PathBuf;
    /// let working_directory = PathBuf::from("/some/temp/path");
    /// let handle = ProcessManager::spawn(working_directory);
    /// ```
    pub fn spawn(working_directory: PathBuf) -> ProcessManagerHandle {
        // TODO: should return result, checking if path exists, is_dir and is writable...
        let (sender, receiver) = mpsc::channel(8);
        let mut manager = Self {
            process_spawner: ProcessSpawner::new(WorkingDir::new(working_directory)),
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
            ProcessManagerMessage::GetProcessInfo { id, responder } => {
                let result = self.get_process_info(id);
                let _ = responder.send(result);
            }
        }
    }

    async fn spawn_process(
        &mut self,
        runnable: Box<dyn Runnable>,
    ) -> Result<ProcessId, SpawnProcessError> {
        let id = ProcessId::random();
        let process = self.process_spawner.spawn_runnable(&id, runnable)?;
        self.processes.insert(id, process);
        Ok(id)
    }

    // TODO: should remove process_dir
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

    fn get_process_info(&mut self, id: ProcessId) -> Result<ProcessInfo, GetProcessInfoError> {
        let process = self
            .processes
            .get_mut(&id)
            .ok_or(GetProcessInfoError::ProcessNotFound(id))?;
        let pid = process.child.id();
        let exit_status = process.child.try_wait()?;
        Ok(ProcessInfo::new(pid, exit_status))
    }
}

/// Type used for communication with [`ProcessManager`] task.
///
/// It provides asynchronous API for:
/// * spawning new child processes, which implements [`Runnable`] trait,
/// * sending messages to spawned processes as:
///     * raw bytes,
///     * strings,
///     * `Rust` data types serialized to raw bytes, `JSON` or `MessagePack`.
/// * receiving messages from spawned processes using asynchronous streams, including:
///     * reading messages by multiple subscribers,
///     * buffering not read messages with configurable buffer capacity,
///     * communication via standard I/O or named pipes.
/// * reading logs produced by spawned processes (from standard output/error streams),
/// * fetching information about spawned processes (OS-assigned pid, exit status),
/// * forcefully killing processes,
/// * waiting processes to finish.
///
/// `ProcessManagerHandle` can only be created by calling [`ProcessManager::spawn`] method.
/// The handle can be cheaply cloned and used safely by many threads.
#[derive(Clone, Debug)]
pub struct ProcessManagerHandle {
    sender: mpsc::Sender<ProcessManagerMessage>,
}

impl ProcessManagerHandle {
    fn new(sender: mpsc::Sender<ProcessManagerMessage>) -> Self {
        Self { sender }
    }

    /// Spawn a new child process, returning its assigned identifier, which can be used to interact with the process.
    /// # Examples
    /// ```
    /// use proc_heim::{manager::ProcessManager, model::command::Cmd};
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let working_dir = tempfile::tempdir()?.into_path();
    /// let handle = ProcessManager::spawn(working_dir);
    /// let cmd = Cmd::new("echo");
    /// let process_id = handle.spawn(cmd).await?;
    /// # Ok(())
    /// # }
    pub async fn spawn(&self, runnable: impl Runnable) -> Result<ProcessId, SpawnProcessError> {
        let (responder, receiver) = oneshot::channel();
        let msg = ProcessManagerMessage::SpawnProcess {
            cmd: Box::new(runnable),
            responder,
        };
        let _ = self.sender.send(msg).await;
        let process_id = receiver.await??;
        Ok(process_id)
    }

    /// Spawn a new child process, returning a [`ProcessHandle`], which can be used to interact with the process.
    pub async fn spawn_with_handle(
        &self,
        runnable: impl Runnable,
    ) -> Result<ProcessHandle, SpawnProcessError> {
        let id = self.spawn(runnable).await?;
        Ok(ProcessHandle::new(id, self.clone()))
    }

    //TODO: merge all subscribe_* methods into special type eg. MessagesSubscriber
    /// Access asynchronous message stream from the process with given `id`.
    ///
    /// Messages will be returned as raw bytes, but with removed new line character (`\n`) from the end of the message.
    /// If message stream was successfully subscribed, then `Ok(stream)` is returned, otherwise a [`ReadMessageError`] is returned.
    /// A stream doesn't yield raw messages, instead each message is wrapped by `Result` with [`ReceiveMessageBytesError`] error type.
    ///
    /// `ProcessManager` for each child process assigns a buffer for messages received from it,
    /// which can be configured via [`CmdOptions::set_message_output_buffer_capacity`](crate::model::command::CmdOptions::set_message_output_buffer_capacity).
    // NOTE: the same doc as in CmdOptions::set_message_output_buffer_capacity below.
    /// When parent process is not reading messages produced by the process,
    /// then the messages are buffered up to the given capacity value.
    /// If the buffer limit is reached and the process sends a new message,
    /// the "oldest" buffered message will be removed. Therefore, when retrieving next message from the stream,
    /// the [`ReceiveMessageBytesError::LostMessages`] error will be returned, indicating how many buffered messages have been removed.
    ///
    /// The messages stream can be subscribed multiple times and each subscriber will receive a one copy of the original message.
    /// Notice that buffer mentioned earlier is created not per subscriber, but per each process, so when one of subscribers not read messages, the buffer will fill up.
    /// # Examples
    /// ```
    /// use futures::TryStreamExt;
    /// use proc_heim::{
    ///     manager::ProcessManager,
    ///     model::command::{Cmd, CmdOptions, MessagingType},
    /// };
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// // spawn ProcessManager
    /// let working_dir = tempfile::tempdir()?.into_path();
    /// let handle = ProcessManager::spawn(working_dir);
    ///
    /// // create simple echo command
    /// let msg = "Hello world!";
    /// let options = CmdOptions::with_message_output(MessagingType::StandardIo);
    /// let cmd = Cmd::with_args_and_options("echo", [msg], options);
    ///
    /// // read a message from spawned process
    /// let process_id = handle.spawn(cmd).await?;
    /// let mut stream = handle.subscribe_message_bytes_stream(process_id).await?;
    /// let received_msg = stream.try_next().await?.unwrap();
    /// assert_eq!(msg.as_bytes(), received_msg);
    /// # Ok(())
    /// # }
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

    /// Access asynchronous message stream from the process with given `id`.
    ///
    /// It works like a [`Self::subscribe_message_bytes_stream`], but the returned stream yields messages of type `T`.
    /// If the message cannot be deserialized, the stream will return [`ReceiveMessageError::CannotDeserializeMessage`] error.
    pub async fn subscribe_message_stream<T, E>(
        &self,
        id: ProcessId,
    ) -> Result<impl Stream<Item = Result<T, ReceiveMessageError>>, ReadMessageError>
    where
        T: TryFrom<Vec<u8>, Error = E>,
        E: Debug,
    {
        Ok(self
            .subscribe_message_bytes_stream(id)
            .await?
            .map(Self::to_message))
    }

    fn to_message<T: TryFrom<Vec<u8>, Error = E>, E: Debug>(
        bytes: Result<Vec<u8>, ReceiveMessageBytesError>,
    ) -> Result<T, ReceiveMessageError> {
        let bytes = bytes?;
        T::try_from(bytes).map_err(|err| {
            ReceiveMessageError::CannotDeserializeMessage(format!(
                "Cannot deserialize data from raw bytes: {err:?}"
            ))
        })
    }

    /// Access asynchronous message stream from the process with given `id`.
    ///
    /// It works like a [`Self::subscribe_message_stream`], but the returned stream yields messages as strings.
    pub async fn subscribe_message_string_stream(
        &self,
        id: ProcessId,
    ) -> Result<impl Stream<Item = Result<String, ReceiveMessageError>>, ReadMessageError> {
        Ok(self
            .subscribe_message_bytes_stream(id)
            .await?
            .map(Self::to_string_message))
    }

    fn to_string_message(
        bytes: Result<Vec<u8>, ReceiveMessageBytesError>,
    ) -> Result<String, ReceiveMessageError> {
        let bytes = bytes?;
        String::from_utf8(bytes).map_err(|err| {
            ReceiveMessageError::CannotDeserializeMessage(format!(
                "Cannot deserialize data from raw bytes to string: {err:?}"
            ))
        })
    }

    // TODO: change name to send_message ?
    /// Send a message to the process with given `id`.
    ///
    /// The message should be serializable to bytes via `TryInto<Vec<u8>` trait.
    /// Notice that `\n` character signals the end of the message,
    /// so message containing `\n` will be truncated to this character.
    /// # Examples
    /// ```
    /// use futures::TryStreamExt;
    /// use proc_heim::{
    ///     manager::ProcessManager,
    ///     model::{
    ///         command::CmdOptions,
    ///         script::{Script, ScriptLanguage},
    ///     },
    /// };
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let working_dir = tempfile::tempdir()?.into_path();
    /// let handle = ProcessManager::spawn(working_dir);
    ///
    /// let script = Script::with_options(
    ///     ScriptLanguage::Bash,
    ///     r#"
    ///     read -r msg < /dev/stdin
    ///     echo "Hello $msg"
    ///     "#,
    ///     CmdOptions::with_standard_io_messaging(),
    /// );
    ///
    /// let process_id = handle.spawn(script).await?;
    /// handle.write_message(process_id, "John").await?;
    /// let mut stream = handle.subscribe_message_string_stream(process_id).await?;
    /// let received_msg = stream.try_next().await?.unwrap();
    /// assert_eq!("Hello John", received_msg);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn write_message<T, E>(&self, id: ProcessId, data: T) -> Result<(), WriteMessageError>
    where
        T: TryInto<Vec<u8>, Error = E>,
        E: Debug,
    {
        let bytes = data.try_into().map_err(|err| {
            WriteMessageError::CannotSerializeMessage(format!(
                "Cannot serialize message to bytes: {err:?}"
            ))
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

    /// Forcefully kills the process with given `id`.
    ///
    /// It will also abort all background tasks related to the process (eg. for messaging, logging) and remove process directory.
    /// # Examples
    /// ```
    /// use proc_heim::{manager::ProcessManager, model::command::Cmd};
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let working_dir = tempfile::tempdir()?.into_path();
    /// let handle = ProcessManager::spawn(working_dir);
    /// let process_id = handle.spawn(Cmd::new("cat")).await?;
    ///
    /// let result = handle.kill(process_id).await;
    ///
    /// assert!(result.is_ok());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn kill(&self, id: ProcessId) -> Result<(), KillProcessError> {
        let (responder, receiver) = oneshot::channel();
        let msg = ProcessManagerMessage::KillProcess { id, responder };
        let _ = self.sender.send(msg).await;
        receiver.await??;
        Ok(())
    }

    /// Fetch logs from standard `output` stream, produced by a process with given `id`.
    ///
    /// The method will return [`GetLogsError::LoggingTypeWasNotConfigured`] error, if [`LoggingType`](enum@crate::model::command::LoggingType) was set to `StderrOnly`.
    /// Notice that each string ending with new line character (`\n`) is treated as one log.
    /// # Examples
    /// ```
    /// use proc_heim::{
    ///     manager::{LogsQuery, ProcessManager},
    ///     model::{
    ///         command::{CmdOptions, LoggingType},
    ///         script::{Script, ScriptLanguage},
    ///     },
    /// };
    /// use std::time::Duration;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let working_dir = tempfile::tempdir()?.into_path();
    /// let handle = ProcessManager::spawn(working_dir);
    /// let script = Script::with_options(
    ///     ScriptLanguage::Bash,
    ///     r#"
    ///     echo message1
    ///     echo message2
    ///     "#,
    ///     CmdOptions::with_logging(LoggingType::StdoutOnly),
    /// );
    ///
    /// let process_id = handle.spawn(script).await?;
    ///
    /// // We need to wait for the process to spawn and finish in order to collect logs,
    /// // otherwise returned logs will be empty.
    /// let _ = handle.wait(process_id, Duration::from_micros(10)).await?;
    ///
    /// let logs = handle
    ///     .get_logs_stdout(process_id, LogsQuery::fetch_all())
    ///     .await?;
    /// assert_eq!(2, logs.len());
    /// assert_eq!("message1", logs[0]);
    /// assert_eq!("message2", logs[1]);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_logs_stdout(
        &self,
        id: ProcessId,
        query: LogsQuery,
    ) -> Result<Vec<String>, GetLogsError> {
        self.get_logs(id, LogsQueryType::Stdout, query).await
    }

    /// Fetch logs from standard `error` stream, produced by a process with given `id`.
    ///
    /// The method will return [`GetLogsError::LoggingTypeWasNotConfigured`] error, if [`LoggingType`](enum@crate::model::command::LoggingType) was set to `StdoutOnly`.
    /// Notice that each string ending with new line character (`\n`) is treated as one log.
    /// # Examples
    /// ```
    /// use proc_heim::{
    ///     manager::{LogsQuery, ProcessManager},
    ///     model::{
    ///         command::{CmdOptions, LoggingType},
    ///         script::{Script, ScriptLanguage},
    ///     },
    /// };
    /// use std::time::Duration;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let working_dir = tempfile::tempdir()?.into_path();
    /// let handle = ProcessManager::spawn(working_dir);
    /// let script = Script::with_options(
    ///     ScriptLanguage::Bash,
    ///     r#"
    ///     echo message1 >&2
    ///     echo message2 >&2
    ///     echo message3 >&2
    ///     "#,
    ///     CmdOptions::with_logging(LoggingType::StderrOnly),
    /// );
    ///
    /// let process_id = handle.spawn(script).await?;
    ///
    /// // We need to wait for the process to spawn and finish in order to collect logs,
    /// // otherwise returned logs will be empty.
    /// let _ = handle.wait(process_id, Duration::from_micros(10)).await?;
    ///
    /// let logs = handle
    ///     .get_logs_stderr(process_id, LogsQuery::with_offset(1))
    ///     .await?;
    /// assert_eq!(2, logs.len());
    /// assert_eq!("message2", logs[0]);
    /// assert_eq!("message3", logs[1]);
    /// # Ok(())
    /// # }
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

    /// Get information about the process with given `id`.
    /// # Examples
    /// Check information of running process:
    /// ```
    /// use proc_heim::{manager::ProcessManager, model::command::Cmd};
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let working_dir = tempfile::tempdir()?.into_path();
    /// let handle = ProcessManager::spawn(working_dir);
    /// let process_id = handle.spawn(Cmd::new("cat")).await?;
    ///
    /// let process_info = handle.get_process_info(process_id).await?;
    /// assert!(process_info.pid().is_some());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_process_info(
        &self,
        id: ProcessId,
    ) -> Result<ProcessInfo, GetProcessInfoError> {
        let (responder, receiver) = oneshot::channel();
        let msg = ProcessManagerMessage::GetProcessInfo { id, responder };
        let _ = self.sender.send(msg).await;
        let data = receiver.await??;
        Ok(data)
    }

    /// Wait for the process with given `id` to finish.
    ///
    /// This method will spawn a `Tokio` task and check in loop, if the process exited.
    /// The `poll_interval` parameter specifies how often to check whether the process has completed or not.
    /// # Examples
    /// ```
    /// use std::time::Duration;
    /// use proc_heim::{manager::ProcessManager, model::command::Cmd};
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let working_dir = tempfile::tempdir()?.into_path();
    /// let handle = ProcessManager::spawn(working_dir);
    /// let process_id = handle.spawn(Cmd::new("env")).await?;
    ///
    /// let process_info = handle.wait(process_id, Duration::from_micros(10)).await??;
    ///
    /// assert!(!process_info.is_running());
    /// assert!(process_info.exit_status().unwrap().success());
    /// # Ok(())
    /// # }
    /// ```
    pub fn wait(
        &self,
        id: ProcessId,
        poll_interval: Duration,
    ) -> JoinHandle<Result<ProcessInfo, GetProcessInfoError>> {
        let handle = self.clone();
        tokio::spawn(async move {
            loop {
                let process_info = handle.get_process_info(id).await?;
                if process_info.is_running() {
                    tokio::time::sleep(poll_interval).await;
                } else {
                    return Ok(process_info);
                }
            }
        })
    }
}

#[cfg(any(feature = "json", feature = "message-pack"))]
use super::serde::{DataFormat, SerdeUtil};

#[cfg(any(feature = "json", feature = "message-pack"))]
impl ProcessManagerHandle {
    /// Access asynchronous message stream from the process with given `id`.
    ///
    /// It works like a [`Self::subscribe_message_stream`], but the returned stream yields messages as deserialized `Rust` data types.
    /// # Examples
    /// ```
    /// use futures::TryStreamExt;
    /// use proc_heim::model::serde::DataFormat;
    /// use proc_heim::{
    ///     manager::ProcessManager,
    ///     model::command::{Cmd, CmdOptions, MessagingType},
    /// };
    /// use serde::{Serialize, Deserialize};
    ///
    /// #[derive(Debug, Deserialize, PartialEq)]
    /// pub struct ExampleMessage {
    ///     pub field1: String,
    ///     pub field2: i32,
    /// }
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///
    /// let working_dir = tempfile::tempdir()?.into_path();
    /// let handle = ProcessManager::spawn(working_dir);
    /// let json_msg = r#"{ "field1": "Hello", "field2": 123 }"#;
    /// let cmd = Cmd::with_args_and_options(
    ///     "echo",
    ///     [json_msg],
    ///     CmdOptions::with_message_output(MessagingType::StandardIo),
    /// );
    ///
    /// let process_id = handle.spawn(cmd).await?;
    ///
    /// let expected_msg = ExampleMessage {
    ///     field1: "Hello".into(),
    ///     field2: 123,
    /// };
    ///
    /// let mut stream = handle
    ///     .subscribe_message_stream_with_format(process_id, DataFormat::Json)
    ///     .await?;
    /// let received_msg = stream.try_next().await?.unwrap();
    /// assert_eq!(expected_msg, received_msg);
    /// Ok(())
    /// }
    /// ```
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

    // TODO: send_message_with_format ?
    // NOTE: example referred from OUTPUT_PIPE_ENV_NAME, INPUT_PIPE_ENV_NAME docs
    /// Send a message (as `Rust` serializable type) to the process with given `id`.
    /// # Examples
    /// ```
    /// use futures::TryStreamExt;
    /// use proc_heim::model::serde::DataFormat;
    /// use proc_heim::{
    ///     manager::ProcessManager,
    ///     model::{
    ///         command::CmdOptions,
    ///         script::{Script, ScriptLanguage},
    ///     },
    /// };
    /// use serde::Serialize;
    ///
    /// #[derive(Debug, Serialize)]
    /// pub struct ExampleMessage {
    ///     pub field1: String,
    ///     pub field2: i32,
    /// }
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let working_dir = tempfile::tempdir()?.into_path();
    ///     let handle = ProcessManager::spawn(working_dir);
    ///     let script = Script::with_options(
    ///         ScriptLanguage::Bash,
    ///         r#"
    ///         read msg < $INPUT_PIPE
    ///         echo "$msg" | jq -r .field1 > $OUTPUT_PIPE
    ///         "#,
    ///         CmdOptions::with_named_pipe_messaging(),
    ///     );
    ///
    ///     let process_id = handle.spawn(script).await?;
    ///
    ///     let msg = ExampleMessage {
    ///         field1: "Some string message".into(),
    ///         field2: 44,
    ///     };
    ///
    ///     handle.write_messages_with_format(process_id, &msg, DataFormat::Json).await?;
    ///
    ///     let mut stream = handle.subscribe_message_string_stream(process_id).await?;
    ///     let received_msg = stream.try_next().await?.unwrap();
    ///     assert_eq!(msg.field1, received_msg);
    ///     Ok(())
    /// }
    /// ```
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

/// Error type returned when reading message bytes from a messages stream.
#[derive(thiserror::Error, Debug)]
pub enum ReceiveMessageBytesError {
    /// Error indicating that some buffered messages were deleted due to buffer capacity overflow.
    /// If you don't care about deleted messages, you can ignore this error.
    /// Includes the number of deleted messages.
    /// See [`ProcessManagerHandle::subscribe_message_bytes_stream`] for more information.
    #[error("Lost {0} number of messages due to buffer capacity overflow")]
    LostMessages(u64),
}

/// Error type returned when reading a deserializable message from a messages stream.
#[derive(thiserror::Error, Debug)]
pub enum ReceiveMessageError {
    /// Error indicating that some buffered messages were deleted due to buffer capacity overflow.
    /// See [`ReceiveMessageBytesError`] for more information.
    #[error("Lost {0} number of messages due to buffer capacity overflow")]
    LostMessages(u64),
    /// Error indicating that a message cannot be deserialized from bytes. Includes error message.
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

/// Error type returned when spawning a new child process.
#[derive(thiserror::Error, Debug)]
pub enum SpawnProcessError {
    /// Cannot communicate with spawned process manager. Probably process manager task has been aborted.
    #[error("Cannot communicate with spawned process manager")]
    ManagerCommunicationError(#[from] oneshot::error::RecvError),
    /// Cannot create named pipes at process directory (first value). Second value is an error code.
    #[error("Cannot create named pipe at path: {0}. Error code: {1}")]
    CannotCreateNamedPipe(PathBuf, String),
    /// Cannot spawn process due to unexpected an IO error.
    #[error("Cannot spawn process: {0}")]
    CannotSpawnProcess(#[from] io::Error),
    /// Cannot create process directory due to an IO error.
    #[error("Cannot create process directory: {0}")]
    CannotCreateProcessWorkingDir(io::Error),
    /// Error was returned by [`Runnable::bootstrap_cmd`] method. Includes an error message returned from this method.
    #[error("Bootstrap process failed: {0}")]
    BootstrapProcessFailed(String),
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
            SpawnerError::BootstrapProcessFailed(err) => Self::BootstrapProcessFailed(err),
        }
    }
}

/// Error type returned when subscribing to a messages stream.
#[derive(thiserror::Error, Debug)]
pub enum ReadMessageError {
    /// The process with given ID was not found (the ID is wrong or the process has been already killed).
    #[error("Process with id: {0} was not found")]
    ProcessNotFound(ProcessId),
    /// Cannot communicate with spawned process manager. Probably process manager task has been aborted.
    #[error("Cannot communicate with spawned process manager")]
    ManagerCommunicationError(#[from] oneshot::error::RecvError),
    /// Cannot get messages stream because a [`MessagingType`](enum@crate::model::command::MessagingType) has not been configured for the process.
    #[error("Message reader for process with id: {0} was not found")]
    MessageReaderNotFound(ProcessId),
    /// The task used to read messages from a process has been killed.
    /// This error "shouldn't" normally occur because the task is aborted when a process is being killed. And after this `ProcessNotFound` will be returned.
    #[error("Message reader process has been killed")]
    MessageReaderKilled,
}

/// Error type returned when writing a message to the process.
#[derive(thiserror::Error, Debug)]
pub enum WriteMessageError {
    /// The process with given ID was not found (the ID is wrong or the process has been already killed).
    #[error("Process with id: {0} was not found")]
    ProcessNotFound(ProcessId),
    /// Cannot serialize message to bytes. Includes error message.
    #[error("{0}")]
    CannotSerializeMessage(String),
    /// Cannot communicate with spawned process manager. Probably process manager task has been aborted.
    #[error("Cannot communicate with spawned process manager")]
    ManagerCommunicationError(#[from] oneshot::error::RecvError),
    /// An unexpected IO error occurred when writing a message to the process.
    #[error("Error occurred when writing message to process: {0}")]
    IoError(#[from] std::io::Error),
    /// The task used to write messages to a process has been killed.
    /// This error "shouldn't" normally occur because the task is aborted when a process is being killed. And after this `ProcessNotFound` will be returned.
    #[error("Message writer for process with id: {0} was not found")]
    MessageWriterNotFound(ProcessId),
}

/// Error type returned when trying to kill the process.
#[derive(thiserror::Error, Debug)]
pub enum KillProcessError {
    /// The process with given ID was not found (the ID is wrong or the process has been already killed).
    #[error("Process with id: {0} was not found")]
    ProcessNotFound(ProcessId),
    /// Cannot communicate with spawned process manager. Probably process manager task has been aborted.
    #[error("Cannot communicate with spawned process manager")]
    ManagerCommunicationError(#[from] oneshot::error::RecvError),
}

/// Error type returned when collecting logs from the process.
#[derive(thiserror::Error, Debug)]
pub enum GetLogsError {
    /// The process with given ID was not found (the ID is wrong or the process has been already killed).
    #[error("Process with id: {0} was not found")]
    ProcessNotFound(ProcessId),
    /// Cannot communicate with spawned process manager. Probably process manager task has been aborted.
    #[error("Cannot communicate with spawned process manager")]
    ManagerCommunicationError(#[from] oneshot::error::RecvError),
    /// Cannot get logs because a proper [`LoggingType`](enum@crate::model::command::MessagingType) has not been configured for the process.
    #[error("Logging type: {0} was not configured for process")]
    LoggingTypeWasNotConfigured(String),
    /// The task used to reading logs from a process has been killed.
    /// This error "shouldn't" normally occur because the task is aborted when a process is being killed. And after this `ProcessNotFound` will be returned.
    #[error("Log read for process with id: {0} was not found")]
    LogReaderNotFound(ProcessId),
    /// An unexpected IO error occurred when reading logs from the process.
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

/// Error type returned when trying to get an information about the process.
#[derive(thiserror::Error, Debug)]
pub enum GetProcessInfoError {
    /// The process with given ID was not found (the ID is wrong or the process has been already killed).
    #[error("Process with id: {0} was not found")]
    ProcessNotFound(ProcessId),
    /// Cannot communicate with spawned process manager. Probably process manager task has been aborted.
    #[error("Cannot communicate with spawned process manager")]
    ManagerCommunicationError(#[from] oneshot::error::RecvError),
    /// An unexpected IO error occurred when trying to get an information about the process.
    #[error(transparent)]
    UnExpectedIoError(#[from] io::Error),
}
