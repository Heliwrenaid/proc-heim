use std::time::Duration;

use crate::manager::{
    GetLogsError, GetProcessDataError, KillProcessError, ProcessManagerHandle, ReadMessageError,
    ReceiveMessageBytesError, ReceiveMessageError, WriteMessageError,
};

use crate::model::process::{LogsQuery, ProcessData, ProcessId};

use tokio::task::JoinHandle;
use tokio_stream::Stream;

pub struct ProcessHandle {
    id: ProcessId,
    handle: ProcessManagerHandle,
}

impl ProcessHandle {
    pub fn new(id: ProcessId, handle: ProcessManagerHandle) -> Self {
        Self { id, handle }
    }

    pub async fn subscribe_message_bytes_stream(
        &self,
    ) -> Result<impl Stream<Item = Result<Vec<u8>, ReceiveMessageBytesError>>, ReadMessageError>
    {
        self.handle.subscribe_message_bytes_stream(self.id).await
    }

    pub async fn subscribe_message_string_stream(
        &self,
    ) -> Result<impl Stream<Item = Result<String, ReceiveMessageError>>, ReadMessageError> {
        self.handle.subscribe_message_string_stream(self.id).await
    }

    pub async fn subscribe_message_stream<T: TryFrom<Vec<u8>>>(
        &self,
    ) -> Result<impl Stream<Item = Result<T, ReceiveMessageError>>, ReadMessageError> {
        self.handle.subscribe_message_stream::<T>(self.id).await
    }

    pub async fn write_message<T: TryInto<Vec<u8>>>(
        &self,
        data: T,
    ) -> Result<(), WriteMessageError> {
        self.handle.write_message::<T>(self.id, data).await
    }

    pub async fn kill(&self) -> Result<(), KillProcessError> {
        self.handle.kill(self.id).await
    }

    pub async fn get_logs_stdout(&self, query: LogsQuery) -> Result<Vec<String>, GetLogsError> {
        self.handle.get_logs_stdout(self.id, query).await
    }

    pub async fn get_logs_stderr(&self, query: LogsQuery) -> Result<Vec<String>, GetLogsError> {
        self.handle.get_logs_stderr(self.id, query).await
    }

    pub async fn get_process_data(&self) -> Result<ProcessData, GetProcessDataError> {
        self.handle.get_process_data(self.id).await
    }

    pub fn wait(
        &self,
        poll_interval: Duration,
    ) -> JoinHandle<Result<ProcessData, GetProcessDataError>> {
        self.handle.wait(self.id, poll_interval)
    }
}

#[cfg(any(feature = "json", feature = "message-pack"))]
use super::serde::DataFormat;

#[cfg(any(feature = "json", feature = "message-pack"))]
impl ProcessHandle {
    pub async fn subscribe_message_stream_with_format<T: serde::de::DeserializeOwned>(
        &self,
        format: DataFormat,
    ) -> Result<impl Stream<Item = Result<T, ReceiveMessageError>>, ReadMessageError> {
        self.handle
            .subscribe_message_stream_with_format::<T>(self.id, format)
            .await
    }

    pub async fn write_messages_with_format<T: serde::Serialize>(
        &self,
        data: T,
        format: DataFormat,
    ) -> Result<(), WriteMessageError> {
        self.handle
            .write_messages_with_format(self.id, data, format)
            .await
    }
}
