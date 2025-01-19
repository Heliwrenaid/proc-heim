use std::{
    collections::HashMap,
    fmt::Debug,
    io,
    path::{Path, PathBuf},
    time::Duration,
};

use tokio::{
    sync::{broadcast, mpsc, oneshot},
    task::JoinHandle,
};
use tokio_stream::{
    wrappers::{errors::BroadcastStreamRecvError, BroadcastStream},
    Stream, StreamExt,
};
use uuid::Uuid;

use crate::working_dir::WorkingDir;

use super::{
    log_reader::{LogReaderError, LogsQuery, LogsQueryType},
    message::Message,
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
/// * spawning new actors (eg. for sending/reading messages to/from user-defined processes),
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
    working_dir: WorkingDir,
    process_spawner: ProcessSpawner,
    processes: HashMap<ProcessId, Process>,
    receiver: mpsc::Receiver<ProcessManagerMessage>,
}

impl ProcessManager {
    /// Spawns a new process manager task with a given `working_directory`, returning a handle associated with the manager's task.
    ///
    /// The `Err` value is returned, if provided `working_directory` is not a directory or is not writeable.
    /// # Examples
    /// ```no_run
    /// # use proc_heim::manager::ProcessManager;
    /// # use std::path::PathBuf;
    /// let working_directory = PathBuf::from("/some/temp/path");
    /// let handle = ProcessManager::spawn(working_directory).expect("Invalid working directory");
    /// ```
    pub fn spawn(working_directory: PathBuf) -> Result<ProcessManagerHandle, io::Error> {
        Self::validate_working_dir(&working_directory)?;
        let (sender, receiver) = mpsc::channel(32);
        let working_dir = WorkingDir::new(working_directory);
        let mut manager = Self {
            working_dir: working_dir.clone(),
            process_spawner: ProcessSpawner::new(working_dir),
            receiver,
            processes: HashMap::new(),
        };
        tokio::spawn(async move { manager.run().await });
        Ok(ProcessManagerHandle::new(sender))
    }

    fn validate_working_dir(working_directory: &Path) -> Result<(), io::Error> {
        let file_path = working_directory.join(Uuid::new_v4().to_string());
        std::fs::write(&file_path, "testing working dir")?;
        std::fs::remove_file(file_path)
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
                let result = self.send_message(id, data).await;
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
        let process_dir = self.working_dir.process_dir(&id);
        let _ = tokio::fs::remove_dir_all(process_dir).await;
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

    async fn send_message(
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
    /// let handle = ProcessManager::spawn(working_dir)?;
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

    /// Send a [`Message`] to the process with given `id`.
    /// # Examples
    /// ```
    /// use futures::TryStreamExt;
    /// use proc_heim::{
    ///     manager::ProcessManager,
    ///     model::{
    ///         command::CmdOptions,
    ///         script::{Script, ScriptingLanguage},
    ///     },
    /// };
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let working_dir = tempfile::tempdir()?.into_path();
    /// let handle = ProcessManager::spawn(working_dir)?;
    ///
    /// let script = Script::with_options(
    ///     ScriptingLanguage::Bash,
    ///     r#"
    ///     read -r msg < /dev/stdin
    ///     echo "Hello $msg"
    ///     "#,
    ///     CmdOptions::with_standard_io_messaging(),
    /// );
    ///
    /// let process_id = handle.spawn(script).await?;
    /// handle.send_message(process_id, "John").await?;
    /// let mut stream = handle.subscribe_message_stream(process_id).await?;
    /// let received_msg = stream.try_next().await?.unwrap();
    /// assert_eq!("Hello John", received_msg.try_into_string().unwrap());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn send_message<M>(&self, id: ProcessId, message: M) -> Result<(), WriteMessageError>
    where
        M: Into<Message>,
    {
        if let Some(data) = Self::into_bytes_with_eol_char(message.into().bytes) {
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

    /// Access asynchronous message stream from the process with given `id`.
    ///
    /// Messages read from the process are returned as [`Message`] types,
    /// allowing a convenient conversion into raw bytes, string or even `Rust` types, which implement `Deserialize` trait.
    /// If message stream was successfully subscribed, then `Ok(stream)` is returned, otherwise a [`ReadMessageError`] is returned.
    /// A stream doesn't yield raw messages, instead each message is wrapped by `Result` with [`ReceiveMessageError`] error type.
    /// For convenient stream transformation use [`TryMessageStreamExt`](trait@crate::manager::TryMessageStreamExt) and [`MessageStreamExt`](trait@crate::manager::MessageStreamExt) traits.
    ///
    /// `ProcessManager` for each child process assigns a buffer for messages received from it,
    /// which can be configured via [`CmdOptions::set_message_output_buffer_capacity`](crate::model::command::CmdOptions::set_message_output_buffer_capacity).
    // NOTE: the same doc as in CmdOptions::set_message_output_buffer_capacity below.
    /// When parent process is not reading messages produced by the process,
    /// then the messages are buffered up to the given capacity value.
    /// If the buffer limit is reached and the process sends a new message,
    /// the "oldest" buffered message will be removed. Therefore, when retrieving next message from the stream,
    /// the [`ReceiveMessageError::LostMessages`] error will be returned, indicating how many buffered messages have been removed.
    ///
    /// The messages stream can be subscribed multiple times and each subscriber will receive a one copy of the original message.
    /// Notice that buffer mentioned earlier is created not per subscriber, but per each process, so when one of subscribers not read messages, the buffer will fill up.
    /// # Examples
    ///
    /// Reading a message via standard IO:
    ///
    /// ```
    /// use futures::TryStreamExt;
    /// use proc_heim::{
    ///     manager::{ProcessManager, TryMessageStreamExt},
    ///     model::command::{Cmd, CmdOptions, MessagingType},
    /// };
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// // spawn ProcessManager
    /// let working_dir = tempfile::tempdir()?.into_path();
    /// let handle = ProcessManager::spawn(working_dir)?;
    ///
    /// // create simple echo command
    /// let msg = "Hello world!";
    /// let options = CmdOptions::with_message_output(MessagingType::StandardIo);
    /// let cmd = Cmd::with_args_and_options("echo", [msg], options);
    ///
    /// // read a message from spawned process
    /// let process_id = handle.spawn(cmd).await?;
    /// let mut stream = handle.subscribe_message_stream(process_id).await?
    ///     .into_string_stream();
    /// let received_msg = stream.try_next().await?.unwrap();
    /// assert_eq!(msg, received_msg);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// Reading messages via named pipes:
    ///
    /// ```
    /// use futures::{StreamExt, TryStreamExt};
    /// use proc_heim::{
    ///     manager::{ProcessManager, TryMessageStreamExt, ResultStreamExt},
    ///     model::{
    ///         command::CmdOptions,
    ///         script::{Script, ScriptingLanguage},
    ///     },
    /// };
    /// use std::path::PathBuf;
    ///
    /// # #[tokio::main] async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let working_dir = tempfile::tempdir()?.into_path();
    /// let handle = ProcessManager::spawn(working_dir)?;
    /// let script = Script::with_options(
    ///     ScriptingLanguage::Bash,
    ///     r#"
    ///     counter=0
    ///     while read msg; do
    ///         echo "$counter: $msg" > $OUTPUT_PIPE
    ///         counter=$((counter + 1))
    ///     done < $INPUT_PIPE
    ///     "#,
    ///     CmdOptions::with_named_pipe_messaging(), // we want to send messages bidirectionally
    /// );
    ///
    /// // We can use "spawn_with_handle" instead of "spawn" to get "ProcessHandle",
    /// // which mimics the "ProcessManagerHandle" API,
    /// // but without having to pass the process ID to each method call.
    /// let process_handle = handle.spawn_with_handle(script).await?;
    ///
    /// process_handle.send_message("First message").await?;
    /// // We can send a next message without causing a deadlock here.
    /// // This is possible because the response to the first message
    /// // will be read by a dedicated Tokio task,
    /// // spawned automatically by the Process Manager.
    /// process_handle.send_message("Second message").await?;
    ///
    /// let mut stream = process_handle
    ///     .subscribe_message_stream()
    ///     .await?
    ///     .into_string_stream();
    ///
    /// assert_eq!("0: First message", stream.try_next().await?.unwrap());
    /// assert_eq!("1: Second message", stream.try_next().await?.unwrap());
    /// # Ok(()) }
    /// ```
    pub async fn subscribe_message_stream(
        &self,
        id: ProcessId,
    ) -> Result<impl Stream<Item = Result<Message, ReceiveMessageError>>, ReadMessageError> {
        let (responder, receiver) = oneshot::channel();
        let msg = ProcessManagerMessage::SubscribeMessageStream { id, responder };
        let _ = self.sender.send(msg).await;
        let message_receiver = receiver.await??;
        let stream = BroadcastStream::new(message_receiver)
            .map(|v| v.map(Message::from_bytes).map_err(Into::into));
        Ok(stream)
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
    ///         script::{Script, ScriptingLanguage},
    ///     },
    /// };
    /// use std::time::Duration;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let working_dir = tempfile::tempdir()?.into_path();
    /// let handle = ProcessManager::spawn(working_dir)?;
    /// let script = Script::with_options(
    ///     ScriptingLanguage::Bash,
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
    ///         script::{Script, ScriptingLanguage},
    ///     },
    /// };
    /// use std::time::Duration;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let working_dir = tempfile::tempdir()?.into_path();
    /// let handle = ProcessManager::spawn(working_dir)?;
    /// let script = Script::with_options(
    ///     ScriptingLanguage::Bash,
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
    /// let handle = ProcessManager::spawn(working_dir)?;
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
    /// let handle = ProcessManager::spawn(working_dir)?;
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

    /// Forcefully kills the process with given `id`.
    ///
    /// It will also abort all background tasks related to the process (eg. for messaging, logging) and remove its process directory.
    /// # Examples
    /// ```
    /// use proc_heim::{manager::ProcessManager, model::command::Cmd};
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let working_dir = tempfile::tempdir()?.into_path();
    /// let handle = ProcessManager::spawn(working_dir)?;
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

    // TODO: abort ProcessManager
}

/// Error type returned when reading message bytes from a messages stream.
#[derive(thiserror::Error, Debug)]
pub enum ReceiveMessageError {
    /// Error indicating that some buffered messages were deleted due to buffer capacity overflow.
    /// If you don't care about deleted messages, you can ignore this error (manually or using [`TryMessageStreamExt::ignore_lost_messages`](crate::manager::TryMessageStreamExt::ignore_lost_messages)).
    /// Includes the number of deleted messages.
    /// See [`ProcessManagerHandle::subscribe_message_stream`] for more information.
    #[error("Lost {0} number of messages due to buffer capacity overflow")]
    LostMessages(u64),
}

/// Error type returned when reading a deserializable message from a messages stream.
#[derive(thiserror::Error, Debug)]
pub enum ReceiveDeserializedMessageError {
    /// Error indicating that some buffered messages were deleted due to buffer capacity overflow.
    /// See [`ReceiveMessageError`] for more information.
    #[error("Lost {0} number of messages due to buffer capacity overflow")]
    LostMessages(u64),
    /// Error indicating that a message cannot be deserialized from bytes. Includes error message.
    #[error("{0}")]
    CannotDeserializeMessage(String),
}

impl From<BroadcastStreamRecvError> for ReceiveMessageError {
    fn from(err: BroadcastStreamRecvError) -> Self {
        match err {
            BroadcastStreamRecvError::Lagged(size) => ReceiveMessageError::LostMessages(size),
        }
    }
}

impl From<ReceiveMessageError> for ReceiveDeserializedMessageError {
    fn from(value: ReceiveMessageError) -> Self {
        match value {
            ReceiveMessageError::LostMessages(number) => {
                ReceiveDeserializedMessageError::LostMessages(number)
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

/// Error type returned when sending a message to the process.
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
    /// An unexpected IO error occurred when sending a message to the process.
    #[error("Error occurred when sending message to process: {0}")]
    IoError(#[from] std::io::Error),
    /// The task used to send messages to a process has been killed.
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
