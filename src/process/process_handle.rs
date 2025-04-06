use std::time::Duration;
use std::{fmt::Debug, sync::Arc};

use crate::manager::{
    GetLogsError, GetProcessInfoError, KillProcessError, LogsQuery, ProcessId, ProcessInfo,
    ProcessManagerHandle, ReadMessageError, ReceiveMessageError, WriteMessageError,
};

use tokio::task::JoinHandle;
use tokio_stream::Stream;

use super::message::Message;

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
    instance_counter: Option<Arc<()>>,
}

impl ProcessHandle {
    /// Creates a new `ProcessHandle` from given process identifier and manager handle.
    pub fn new(id: ProcessId, handle: ProcessManagerHandle) -> Self {
        Self {
            id,
            handle,
            instance_counter: None,
        }
    }

    /// Like a [`Self::new`], but dropping handle returned by this method will kill the child process managed by that handle.
    /// If the handle has been cloned, the child process will terminate when the last instance of the handle is dropped.
    /// There is no guarantee that the process will be completed immediately after the handle is dropped.
    pub fn new_scoped(id: ProcessId, handle: ProcessManagerHandle) -> Self {
        Self {
            id,
            handle,
            instance_counter: Some(Arc::new(())),
        }
    }

    /// Returns a process identifier associated with a handle.
    pub fn id(&self) -> &ProcessId {
        &self.id
    }

    /// See [`ProcessManagerHandle::send_message`] docs.
    pub async fn send_message<M>(&self, message: M) -> Result<(), WriteMessageError>
    where
        M: Into<Message>,
    {
        self.handle.send_message(self.id, message).await
    }

    /// See [`ProcessManagerHandle::subscribe_message_stream`] docs.
    pub async fn subscribe_message_stream(
        &self,
    ) -> Result<impl Stream<Item = Result<Message, ReceiveMessageError>>, ReadMessageError> {
        self.handle.subscribe_message_stream(self.id).await
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

impl Drop for ProcessHandle {
    fn drop(&mut self) {
        if self.instance_counter.is_some()
            && Arc::strong_count(&self.instance_counter.take().unwrap()) == 1
            && !self.handle.try_kill(self.id)
        {
            let mut handle = self.clone();
            handle.instance_counter = None;
            tokio::spawn(async move {
                handle
                    .handle
                    .kill_with_timeout(handle.id, Duration::from_millis(200))
                    .await
            });
        }
    }
}
