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

/// Type representing information about spawned process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessInfo {
    pid: Option<u32>,
    exit_status: Option<ExitStatus>,
}

impl ProcessInfo {
    pub(crate) fn new(pid: Option<u32>, exit_status: Option<ExitStatus>) -> Self {
        Self { pid, exit_status }
    }

    /// Retrieve information about OS-assigned process identifier. For more information see [`Child::id`](tokio::process::Child::id) docs.
    pub fn pid(&self) -> Option<u32> {
        self.pid
    }

    /// Retrieve information about process exit status.
    /// If the process has exited, then `Some(status)` is returned.
    /// If the exit status is not available at this time then `None` is returned.
    pub fn exit_status(&self) -> Option<ExitStatus> {
        self.exit_status
    }

    /// Check if process is still running. This is equivalent of `self.exit_status().is_none()`.
    pub fn is_running(&self) -> bool {
        self.exit_status.is_none()
    }
}
