use std::time::Duration;
use std::{fmt::Debug, sync::Arc};

use crate::manager::{
    GetLogsError, GetProcessInfoError, KillProcessError, LogsQuery, ProcessId, ProcessInfo,
    ReadMessageError, ReceiveMessageError, WriteMessageError,
};

use tokio::task::JoinHandle;
use tokio_stream::Stream;

use super::message::Message;
use super::ProcessHandle;

/// A wrapper around [`ProcessHandle`] that kills the associated child process, when the last instance of the handle is dropped.
///
/// There is no guarantee that the process will be killed immediately.
///
/// The handle can be created also by calling [`ProcessManagerHandle::spawn_with_scoped_handle`](method@crate::process::ProcessManagerHandle::spawn_with_scoped_handle).
#[derive(Clone, Debug)]
pub struct ScopedProcessHandle {
    handle: ProcessHandle,
    instance_counter: Arc<()>,
}

impl ScopedProcessHandle {
    /// Creates a new `ScopedProcessHandle` from given process handle.
    pub fn new(handle: ProcessHandle) -> Self {
        Self {
            handle,
            instance_counter: Arc::new(()),
        }
    }

    /// Returns a process identifier associated with a handle.
    pub fn id(&self) -> &ProcessId {
        self.handle.id()
    }

    /// See [`ProcessManagerHandle::send_message`](method@crate::process::ProcessManagerHandle::send_message) docs.
    pub async fn send_message<M>(&self, message: M) -> Result<(), WriteMessageError>
    where
        M: Into<Message>,
    {
        self.handle.send_message(message).await
    }

    /// See [`ProcessManagerHandle::subscribe_message_stream`](method@crate::process::ProcessManagerHandle::subscribe_message_stream) docs.
    pub async fn subscribe_message_stream(
        &self,
    ) -> Result<impl Stream<Item = Result<Message, ReceiveMessageError>>, ReadMessageError> {
        self.handle.subscribe_message_stream().await
    }

    /// See [`ProcessManagerHandle::get_logs_stdout`](method@crate::process::ProcessManagerHandle::get_logs_stdout) docs.
    pub async fn get_logs_stdout(&self, query: LogsQuery) -> Result<Vec<String>, GetLogsError> {
        self.handle.get_logs_stdout(query).await
    }

    /// See [`ProcessManagerHandle::get_logs_stderr`](method@crate::process::ProcessManagerHandle::get_logs_stderr) docs.
    pub async fn get_logs_stderr(&self, query: LogsQuery) -> Result<Vec<String>, GetLogsError> {
        self.handle.get_logs_stderr(query).await
    }

    /// See [`ProcessManagerHandle::get_process_info`](method@crate::process::ProcessManagerHandle::get_process_info) docs.
    pub async fn get_process_info(&self) -> Result<ProcessInfo, GetProcessInfoError> {
        self.handle.get_process_info().await
    }

    /// See [`ProcessManagerHandle::wait`](method@crate::process::ProcessManagerHandle::wait) docs.
    pub fn wait(
        &self,
        poll_interval: Duration,
    ) -> JoinHandle<Result<ProcessInfo, GetProcessInfoError>> {
        self.handle.wait(poll_interval)
    }

    /// See [`ProcessManagerHandle::kill`](method@crate::process::ProcessManagerHandle::kill) docs.
    pub async fn kill(&self) -> Result<(), KillProcessError> {
        self.handle.kill().await
    }
}

impl Drop for ScopedProcessHandle {
    fn drop(&mut self) {
        if Arc::strong_count(&self.instance_counter) == 1
            && !self.handle.handle.try_kill(*self.id())
        {
            let handle = self.handle.clone();
            tokio::spawn(async move {
                handle
                    .handle
                    .kill_with_timeout(*handle.id(), Duration::from_millis(200))
                    .await
            });
        }
    }
}

impl From<ProcessHandle> for ScopedProcessHandle {
    fn from(handle: ProcessHandle) -> Self {
        Self::new(handle)
    }
}
