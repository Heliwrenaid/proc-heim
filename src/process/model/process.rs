use std::process::ExitStatus;

use derive_builder::Builder;
use tokio::process::Child;

use crate::process::{
    log_reader::LogReaderHandle, reader::MessageReaderHandle, writer::MessageWriterHandle,
};

#[derive(Builder, Debug)]
#[builder(pattern = "owned")]
pub struct Process {
    pub child: Child,
    #[builder(default = "None")]
    pub message_writer: Option<MessageWriterHandle>,
    #[builder(default = "None")]
    pub message_reader: Option<MessageReaderHandle>,
    #[builder(default = "None")]
    pub log_reader: Option<LogReaderHandle>,
}

#[derive(Debug, Clone)]
pub struct ProcessData {
    pid: Option<u32>,
    exit_status: Option<ExitStatus>,
}

impl ProcessData {
    pub(crate) fn new(pid: Option<u32>, exit_status: Option<ExitStatus>) -> Self {
        Self { pid, exit_status }
    }

    pub fn pid(&self) -> Option<u32> {
        self.pid
    }

    pub fn exit_status(&self) -> Option<ExitStatus> {
        self.exit_status
    }

    pub fn is_running(&self) -> bool {
        self.exit_status.is_none()
    }
}
