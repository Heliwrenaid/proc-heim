use std::path::PathBuf;

use nix::{sys::stat, unistd};
use proc_heim::{Cmd, CmdBuilder};
use tempfile::{tempdir, TempDir};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::unix::pipe,
};
use uuid::Uuid;

pub struct TestPipe {
    pub _tmp_dir: TempDir,
    pub pipe_path: PathBuf,
}

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

    pub fn writer(&self) -> impl AsyncWriteExt {
        pipe::OpenOptions::new()
            .read_write(true)
            .open_sender(&self.pipe_path)
            .unwrap()
    }

    pub fn reader(&self) -> impl AsyncReadExt {
        pipe::OpenOptions::new()
            .read_write(true)
            .open_receiver(&self.pipe_path)
            .unwrap()
    }
}

pub fn echo_cmd(text: &str) -> Cmd {
    CmdBuilder::default()
        .cmd("echo".into())
        .args(vec!["-n".into(), text.into()].into())
        .build()
        .unwrap()
}

pub fn cat_cmd() -> Cmd {
    CmdBuilder::default().cmd("cat".into()).build().unwrap()
}
