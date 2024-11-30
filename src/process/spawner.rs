use std::{io, path::PathBuf};

use nix::{errno::Errno, sys::stat, unistd};
use tokio::{net::unix, process::Command};

use crate::working_dir::WorkingDir;

use super::{
    model::{MessagingType, Process, ProcessBuilder},
    reader::{MessageReader, MessageReaderError},
    writer::MessageWriter,
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

        if let Some(current_dir) = cmd.options.current_dir {
            child.current_dir(current_dir);
        }

        if cmd.options.clear_envs {
            child.env_clear();
        }

        if let Some(envs) = cmd.options.envs {
            child.envs(envs);
        }

        let mut process_builder = ProcessBuilder::default();

        if let Some(messaging_type) = cmd.options.message_output {
            let receiver = match messaging_type {
                MessagingType::StandardIo => {
                    let (sender, receiver) = unix::pipe::pipe().unwrap();
                    child.stdout(sender.into_nonblocking_fd()?);
                    receiver
                }
                MessagingType::NamedPipe => {
                    let output_pipe = self.working_dir.message_reader_pipe(id);
                    Self::create_named_pipe(&output_pipe)?;
                    child.env("OUTPUT_PIPE", output_pipe.clone());
                    let receiver = unix::pipe::OpenOptions::new()
                        .read_write(true)
                        .open_receiver(output_pipe)?;
                    receiver
                }
            };
            let message_reader = MessageReader::new(receiver, cmd.options.output_buffer_capacity)?;
            process_builder = process_builder.message_reader(message_reader.into())
        }

        if let Some(messaging_type) = cmd.options.message_input {
            let sender = match messaging_type {
                MessagingType::StandardIo => {
                    let (sender, receiver) = unix::pipe::pipe().unwrap();
                    child.stdin(receiver.into_nonblocking_fd()?);
                    sender
                }
                MessagingType::NamedPipe => {
                    let input_pipe = self.working_dir.message_writer_pipe(id);
                    Self::create_named_pipe(&input_pipe)?;
                    child.env("INPUT_PIPE", input_pipe.clone());
                    let sender = unix::pipe::OpenOptions::new()
                        .read_write(true)
                        .open_sender(input_pipe)?;
                    sender
                }
            };
            process_builder = process_builder.message_writer(MessageWriter::new(sender)?.into())
        }

        let child_handle = child.spawn()?;

        Ok(process_builder.child(child_handle).build().unwrap())
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
    #[error("Invalid output buffer capacity: {0}")]
    InvalidOutputBufferCapacity(usize),
}

impl From<MessageReaderError> for SpawnerError {
    fn from(err: MessageReaderError) -> Self {
        match err {
            MessageReaderError::InvalidChannelCapacity(value) => {
                Self::InvalidOutputBufferCapacity(value)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::CmdBuilder;

    use super::*;

    use sysinfo::{Pid, System};
    use tempfile::env::temp_dir;

    #[tokio::test]
    async fn should_spawn_process() {
        let working_dir = WorkingDir::new(temp_dir());
        let spawner = ProcessSpawner::new(working_dir);

        let id = ProcessId::random();
        let result = spawner.spawn(&id, cat_cmd());
        assert!(result.is_ok());
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

    fn cat_cmd() -> Cmd {
        CmdBuilder::default().cmd("cat".into()).build().unwrap()
    }

    // TODO: test setting other props and error handling
}
