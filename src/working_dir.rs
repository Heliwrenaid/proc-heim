use std::{fs, io, path::PathBuf};

use crate::process::ProcessId;

pub struct WorkingDir {
    dir: PathBuf,
}

impl WorkingDir {
    pub(crate) fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    pub(crate) fn message_writer_pipe(&self, id: &ProcessId) -> PathBuf {
        self.dir.join(id.to_string()).join("writer_pipe")
    }

    pub(crate) fn message_reader_pipe(&self, id: &ProcessId) -> PathBuf {
        self.dir.join(id.to_string()).join("reader_pipe")
    }

    pub(crate) fn create_process_dir(&self, id: &ProcessId) -> io::Result<()> {
        fs::create_dir(self.dir.join(id.to_string()))
    }
}
