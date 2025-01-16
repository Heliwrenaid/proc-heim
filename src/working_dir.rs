use std::path::PathBuf;

use crate::process::ProcessId;

#[derive(Clone, Debug)]
pub(crate) struct WorkingDir {
    dir: PathBuf,
}

impl WorkingDir {
    pub(crate) fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    pub(crate) fn process_dir(&self, id: &ProcessId) -> PathBuf {
        self.dir.join(id.to_string())
    }

    pub(crate) fn process_data_dir(&self, id: &ProcessId) -> PathBuf {
        self.process_dir(id).join("data")
    }

    pub(crate) fn message_writer_pipe(&self, id: &ProcessId) -> PathBuf {
        self.process_dir(id).join("writer_pipe")
    }

    pub(crate) fn message_reader_pipe(&self, id: &ProcessId) -> PathBuf {
        self.process_dir(id).join("reader_pipe")
    }

    pub(crate) fn logs_stdout(&self, id: &ProcessId) -> PathBuf {
        self.process_dir(id).join("stdout_logs")
    }

    pub(crate) fn logs_stderr(&self, id: &ProcessId) -> PathBuf {
        self.process_dir(id).join("stderr_logs")
    }

    pub(crate) fn logs_merged(&self, id: &ProcessId) -> PathBuf {
        self.process_dir(id).join("logs")
    }
}
