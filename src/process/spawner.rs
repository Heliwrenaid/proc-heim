use std::{io, path::PathBuf, process::Stdio};

use nix::{errno::Errno, sys::stat, unistd};
use tokio::process::Command;

use crate::working_dir::WorkingDir;

use super::{
    model::{Process, ProcessBuilder},
    Cmd, ProcessId,
};

pub struct ProcessSpawner {
    working_dir: WorkingDir,
}

impl ProcessSpawner {
    pub fn new(working_dir: WorkingDir) -> Self {
        Self { working_dir }
    }

    pub fn spawn(&self, id: &ProcessId, cmd: Cmd) -> Result<Process, SpawnerError> {
        self.working_dir
            .create_process_dir(id)
            .map_err(SpawnerError::CannotCreateProcessWorkingDir)?;
        let mut child = Command::new(cmd.cmd);
        if let Some(args) = cmd.args {
            child.args(args);
        }
        child.kill_on_drop(true);
        child.stdout(Stdio::piped()); // now setting for tests, in the future will be used to collect "logs"

        if let Some(current_dir) = cmd.options.current_dir {
            child.current_dir(current_dir);
        }

        if cmd.options.clear_envs {
            child.env_clear();
        }

        if let Some(envs) = cmd.options.envs {
            child.envs(envs);
        }

        let input_pipe = self.working_dir.message_writer_pipe(id);
        Self::create_named_pipe(&input_pipe)?;
        child.env("INPUT_PIPE", input_pipe.clone());

        let output_pipe = self.working_dir.message_reader_pipe(id);
        Self::create_named_pipe(&output_pipe)?;
        child.env("OUTPUT_PIPE", output_pipe.clone());

        let child_handle = child.spawn()?;

        Ok(ProcessBuilder::default()
            .child(child_handle)
            .input_pipe(input_pipe.into())
            .output_pipe(output_pipe.into())
            .build()
            .unwrap())
    }

    fn create_named_pipe(pipe_path: &PathBuf) -> Result<(), SpawnerError> {
        unistd::mkfifo(pipe_path, stat::Mode::S_IRWXU)
            .map_err(|err| SpawnerError::CannotCreateNamedPipe(pipe_path.clone(), err))
    }
}

#[derive(thiserror::Error, Debug)]
pub enum SpawnerError {
    #[error("Cannot create named pipe at path: {0}. Error code: {1}")]
    CannotCreateNamedPipe(PathBuf, Errno),
    #[error("Cannot spawn process: {0}")]
    CannotSpawnProcess(#[from] io::Error),
    #[error("Cannot spawn process: {0}")]
    CannotCreateProcessWorkingDir(io::Error),
}

#[cfg(test)]
mod tests {
    use std::{path::Path, time::Duration};

    use crate::CmdBuilder;

    use super::*;
    use sysinfo::{Pid, System};
    use tempfile::env::temp_dir;
    use tokio::{io::AsyncReadExt, net::unix::pipe};

    #[tokio::test]
    async fn should_spawn_process() {
        let msg = "Hey! Listen";
        let cmd = echo_cmd(msg);
        let working_dir = WorkingDir::new(temp_dir());
        let spawner = ProcessSpawner::new(working_dir);

        let id = ProcessId::random();
        let process = spawner.spawn(&id, cmd).unwrap();

        assert_pipe_exists(&process.input_pipe.unwrap());
        assert_pipe_exists(&process.output_pipe.unwrap());
        let mut stdout = process.child.stdout.unwrap();
        let mut echo_msg: String = String::new();
        stdout.read_to_string(&mut echo_msg).await.unwrap();
        assert_eq!(msg, &echo_msg);
    }

    #[tokio::test]
    async fn should_kill_process_on_drop() {
        let cmd = cat_cmd();
        let working_dir = WorkingDir::new(temp_dir());
        let spawner = ProcessSpawner::new(working_dir);

        let id = ProcessId::random();
        let process = spawner.spawn(&id, cmd).unwrap();
        let pid = process.child.id().unwrap();
        let pid = Pid::from_u32(pid);

        let sys = System::new_all();
        assert!(sys.process(pid).is_some());

        drop(process);
        tokio::time::sleep(Duration::from_secs(1)).await;

        let sys = System::new_all();
        assert!(sys.process(pid).is_none());
    }

    fn echo_cmd(text: &str) -> Cmd {
        CmdBuilder::default()
            .cmd("echo".into())
            .args(vec!["-n".into(), text.into()].into())
            .build()
            .unwrap()
    }

    fn cat_cmd() -> Cmd {
        CmdBuilder::default().cmd("cat".into()).build().unwrap()
    }

    fn assert_pipe_exists(pipe_path: &Path) {
        let result = pipe::OpenOptions::new()
            .unchecked(false) // check if file has FIFO type
            .read_write(true)
            .open_receiver(pipe_path);
        assert!(result.is_ok());
    }

    // TODO: test error handling
}
