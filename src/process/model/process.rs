use std::process::ExitStatus;

use tokio::process::Child;

use crate::process::{
    log_reader::LogReaderHandle, reader::MessageReaderHandle, writer::MessageWriterHandle,
};

#[derive(Debug)]
pub(crate) struct Process {
    pub child: Child,
    pub message_writer: Option<MessageWriterHandle>,
    pub message_reader: Option<MessageReaderHandle>,
    pub log_reader: Option<LogReaderHandle>,
}

#[derive(Debug, Default)]
pub(crate) struct ProcessBuilder {
    pub message_writer: Option<MessageWriterHandle>,
    pub message_reader: Option<MessageReaderHandle>,
    pub log_reader: Option<LogReaderHandle>,
}

impl ProcessBuilder {
    pub fn build(self, child: Child) -> Process {
        Process {
            child,
            message_writer: self.message_writer,
            message_reader: self.message_reader,
            log_reader: self.log_reader,
        }
    }
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
