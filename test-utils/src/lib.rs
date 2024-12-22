pub mod cmd_collection;
pub mod scripts_collection;

use std::path::PathBuf;

use nix::{sys::stat, unistd};
use tempfile::{tempdir, TempDir};
use tokio::net::unix::pipe;
use uuid::Uuid;

pub struct TestPipe {
    pub _tmp_dir: TempDir,
    pub pipe_path: PathBuf,
}

#[allow(clippy::new_without_default)]
impl TestPipe {
    pub fn new() -> Self {
        let tmp_dir = tempdir().unwrap();
        let pipe_name = tmp_dir.path().join(Uuid::new_v4().to_string());
        unistd::mkfifo(&pipe_name, stat::Mode::S_IRWXU).unwrap();
        Self {
            _tmp_dir: tmp_dir,
            pipe_path: pipe_name,
        }
    }

    pub fn writer(&self) -> pipe::Sender {
        pipe::OpenOptions::new()
            .read_write(true)
            .open_sender(&self.pipe_path)
            .unwrap()
    }

    pub fn reader(&self) -> pipe::Receiver {
        pipe::OpenOptions::new()
            .read_write(true)
            .open_receiver(&self.pipe_path)
            .unwrap()
    }
}
