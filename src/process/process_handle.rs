use std::fmt::Debug;
use std::time::Duration;

use crate::manager::{
    GetLogsError, GetProcessInfoError, KillProcessError, LogsQuery, ProcessId, ProcessInfo,
    ProcessManagerHandle, ReadMessageError, ReceiveMessageBytesError, ReceiveMessageError,
    WriteMessageError,
};

use tokio::task::JoinHandle;
use tokio_stream::Stream;

/// `ProcessManagerHandle` wrapper used to interact with only one spawned process.
///
/// `ProcessHandle` mimics [`ProcessManagerHandle`] API, but does not require passing [`ProcessId`] as methods parameter.
/// It can be created by [`ProcessManagerHandle::spawn_with_handle`] method or manually by [`ProcessHandle::new`].
/// Like a `ProcessManagerHandle` it can be cheaply cloned and used by many threads safely.
///
/// See [`ProcessManagerHandle`] docs for more information.
#[derive(Clone, Debug)]
pub struct ProcessHandle {
    id: ProcessId,
    handle: ProcessManagerHandle,
}

impl ProcessHandle {
    /// Creates a new `ProcessHandle` from given process identifier and manager handle.
    pub fn new(id: ProcessId, handle: ProcessManagerHandle) -> Self {
        Self { id, handle }
    }

    /// Returns a process identifier associated with a handle.
    pub fn id(&self) -> &ProcessId {
        &self.id
    }

    /// See [`ProcessManagerHandle::send_message`] docs.
    pub async fn send_message<T, E>(&self, data: T) -> Result<(), WriteMessageError>
    where
        T: TryInto<Vec<u8>, Error = E>,
        E: Debug,
    {
        self.handle.send_message::<T, E>(self.id, data).await
    }

    /// See [`ProcessManagerHandle::subscribe_message_bytes_stream`] docs.
    pub async fn subscribe_message_bytes_stream(
        &self,
    ) -> Result<impl Stream<Item = Result<Vec<u8>, ReceiveMessageBytesError>>, ReadMessageError>
    {
        self.handle.subscribe_message_bytes_stream(self.id).await
    }

    /// See [`ProcessManagerHandle::subscribe_message_string_stream`] docs.
    pub async fn subscribe_message_string_stream(
        &self,
    ) -> Result<impl Stream<Item = Result<String, ReceiveMessageError>>, ReadMessageError> {
        self.handle.subscribe_message_string_stream(self.id).await
    }

    /// See [`ProcessManagerHandle::subscribe_message_stream`] docs.
    pub async fn subscribe_message_stream<T, E>(
        &self,
    ) -> Result<impl Stream<Item = Result<T, ReceiveMessageError>>, ReadMessageError>
    where
        T: TryFrom<Vec<u8>, Error = E>,
        E: Debug,
    {
        self.handle.subscribe_message_stream::<T, E>(self.id).await
    }

    /// See [`ProcessManagerHandle::get_logs_stdout`] docs.
    pub async fn get_logs_stdout(&self, query: LogsQuery) -> Result<Vec<String>, GetLogsError> {
        self.handle.get_logs_stdout(self.id, query).await
    }

    /// See [`ProcessManagerHandle::get_logs_stderr`] docs.
    pub async fn get_logs_stderr(&self, query: LogsQuery) -> Result<Vec<String>, GetLogsError> {
        self.handle.get_logs_stderr(self.id, query).await
    }

    /// See [`ProcessManagerHandle::get_process_info`] docs.
    pub async fn get_process_info(&self) -> Result<ProcessInfo, GetProcessInfoError> {
        self.handle.get_process_info(self.id).await
    }

    /// See [`ProcessManagerHandle::wait`] docs.
    pub fn wait(
        &self,
        poll_interval: Duration,
    ) -> JoinHandle<Result<ProcessInfo, GetProcessInfoError>> {
        self.handle.wait(self.id, poll_interval)
    }

    /// See [`ProcessManagerHandle::kill`] docs.
    pub async fn kill(&self) -> Result<(), KillProcessError> {
        self.handle.kill(self.id).await
    }
}

#[cfg(any(feature = "json", feature = "message-pack"))]
use super::serde::MessageFormat;

#[cfg(any(feature = "json", feature = "message-pack"))]
impl ProcessHandle {
    /// See [`ProcessManagerHandle::subscribe_message_stream_with_format`] docs.
    pub async fn subscribe_message_stream_with_format<T: serde::de::DeserializeOwned>(
        &self,
        format: MessageFormat,
    ) -> Result<impl Stream<Item = Result<T, ReceiveMessageError>>, ReadMessageError> {
        self.handle
            .subscribe_message_stream_with_format::<T>(self.id, format)
            .await
    }

    /// See [`ProcessManagerHandle::send_message_with_format`] docs.
    pub async fn send_message_with_format<T: serde::Serialize>(
        &self,
        data: T,
        format: MessageFormat,
    ) -> Result<(), WriteMessageError> {
        self.handle
            .send_message_with_format(self.id, data, format)
            .await
    }
}
