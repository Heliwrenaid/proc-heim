use std::{
    fs::{self, File},
    io,
    path::{Path, PathBuf},
    process::Stdio,
};

use nix::{
    errno::Errno,
    sys::stat::{self},
    unistd,
};
use tokio::{net::unix, process::Command};

use crate::working_dir::WorkingDir;

use super::{
    log_reader::LogReader,
    model::{MessagingType, Process, ProcessBuilder},
    reader::MessageReader,
    writer::MessageWriter,
    Cmd, LoggingType, ProcessId, Runnable,
};

/// Environment variable representing the directory intended for storing temporary process data.
///
/// The directory will be deleted when the process is killed manually.
pub const PROCESS_DATA_DIR_ENV_NAME: &str = "PROCESS_DATA_DIR";
/// Environment variable representing a named pipe path used to read incoming messages in the child process.
///
/// See [`ProcessManagerHandle::send_message_with_format`](crate::manager::ProcessManagerHandle::send_message_with_format) example.
pub const INPUT_PIPE_ENV_NAME: &str = "INPUT_PIPE";
/// Environment variable representing a named pipe path used to send messages to the parent process.
///
/// See [`ProcessManagerHandle::send_message_with_format`](crate::manager::ProcessManagerHandle::send_message_with_format) example.
pub const OUTPUT_PIPE_ENV_NAME: &str = "OUTPUT_PIPE";

pub struct ProcessSpawner {
    working_dir: WorkingDir,
}

impl ProcessSpawner {
    pub fn new(working_dir: WorkingDir) -> Self {
        Self { working_dir }
    }

    pub fn spawn_runnable(
        &self,
        id: &ProcessId,
        runnable: Box<dyn Runnable>,
    ) -> Result<Process, SpawnerError> {
        let process_dir = self.working_dir.process_dir(id);
        fs::create_dir(&process_dir).map_err(SpawnerError::CannotCreateProcessWorkingDir)?;
        fs::create_dir(self.working_dir.process_data_dir(id))?;
        match self.try_spawn_runnable(id, &*runnable, &process_dir) {
            Ok(process) => Ok(process),
            Err(err) => {
                let _ = runnable.clean_after_fail(&process_dir);
                let _ = fs::remove_dir_all(&process_dir);
                Err(err)
            }
        }
    }

    fn try_spawn_runnable(
        &self,
        id: &ProcessId,
        runnable: &dyn Runnable,
        process_dir: &Path,
    ) -> Result<Process, SpawnerError> {
        let cmd = runnable
            .bootstrap_cmd(process_dir)
            .map_err(SpawnerError::BootstrapProcessFailed)?;
        self.spawn(id, cmd)
    }

    fn spawn(&self, id: &ProcessId, cmd: Cmd) -> Result<Process, SpawnerError> {
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

        if let Some(envs) = cmd.options.envs_to_remove {
            for env in envs {
                child.env_remove(env);
            }
        }

        if let Some(envs) = cmd.options.envs {
            child.envs(envs);
        }

        let mut process_builder = ProcessBuilder::default();

        child.env(
            PROCESS_DATA_DIR_ENV_NAME,
            self.working_dir.process_data_dir(id),
        );

        child.stdin(Stdio::null());
        child.stdout(Stdio::null());
        child.stderr(Stdio::null());

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
                    child.env(OUTPUT_PIPE_ENV_NAME, output_pipe.clone());
                    unix::pipe::OpenOptions::new()
                        .read_write(true)
                        .open_receiver(output_pipe)?
                }
            };
            let message_reader = MessageReader::spawn(receiver, cmd.options.output_buffer_capacity);
            process_builder.message_reader = message_reader.into();
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
                    child.env(INPUT_PIPE_ENV_NAME, input_pipe.clone());
                    unix::pipe::OpenOptions::new()
                        .read_write(true)
                        .open_sender(input_pipe)?
                }
            };
            process_builder.message_writer = MessageWriter::spawn(sender)?.into()
        }

        if let Some(ref logging_type) = cmd.options.logging_type {
            match logging_type {
                LoggingType::StdoutOnly => {
                    let path = self.working_dir.logs_stdout(id);
                    let file = Self::create_log_file(&path)?;
                    child.stdout(file);
                    let reader = LogReader::spawn(path.into(), None, None);
                    process_builder.log_reader = reader.into();
                }
                LoggingType::StderrOnly => {
                    let path = self.working_dir.logs_stderr(id);
                    let file = Self::create_log_file(&path)?;
                    child.stderr(file);
                    let reader = LogReader::spawn(None, path.into(), None);
                    process_builder.log_reader = reader.into();
                }
                LoggingType::StdoutAndStderr => {
                    let stdout_path = self.working_dir.logs_stdout(id);
                    let file = Self::create_log_file(&stdout_path)?;
                    child.stdout(file);

                    let stderr_path = self.working_dir.logs_stderr(id);
                    let file = Self::create_log_file(&stderr_path)?;
                    child.stderr(file);
                    let reader = LogReader::spawn(stdout_path.into(), stderr_path.into(), None);
                    process_builder.log_reader = reader.into();
                }
                LoggingType::StdoutAndStderrMerged => {
                    let path = self.working_dir.logs_merged(id);
                    let file1 = Self::create_log_file(&path)?;
                    let file2 = file1.try_clone()?;
                    child.stdout(file1);
                    child.stderr(file2);
                    let reader = LogReader::spawn(None, None, path.into());
                    process_builder.log_reader = reader.into();
                }
            }
        }

        let child_handle = child.spawn()?;
        Ok(process_builder.build(child_handle))
    }

    fn create_named_pipe(pipe_path: &PathBuf) -> Result<(), SpawnerError> {
        unistd::mkfifo(pipe_path, stat::Mode::S_IRWXU)
            .map_err(|err| SpawnerError::CannotCreateNamedPipe(pipe_path.clone(), err))
    }

    fn create_log_file(path: &PathBuf) -> Result<File, SpawnerError> {
        let file = File::options().append(true).create(true).open(path)?;
        Ok(file)
    }
}

#[derive(thiserror::Error, Debug)]
pub enum SpawnerError {
    #[error("Cannot create named pipe at path: {0}. Error code: {1}")]
    CannotCreateNamedPipe(PathBuf, Errno),
    #[error("Cannot spawn process: {0}")]
    CannotSpawnProcess(#[from] io::Error),
    #[error("Cannot create process directory: {0}")]
    CannotCreateProcessWorkingDir(io::Error),
    #[error("Bootstrap process failed: {0}")]
    BootstrapProcessFailed(String),
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::process::{log_reader::LogSettingsQuery, CmdOptions};

    use super::*;

    use sysinfo::{Pid, System};
    use tempfile::env::temp_dir;

    #[tokio::test]
    async fn should_spawn_process() {
        let working_dir = WorkingDir::new(temp_dir());
        let spawner = ProcessSpawner::new(working_dir.clone());

        let id = ProcessId::random();
        let result = spawner.spawn_runnable(&id, Box::new(cat_cmd()));
        assert!(working_dir.process_dir(&id).exists());
        assert!(working_dir.process_data_dir(&id).exists());
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

    #[tokio::test]
    async fn should_setup_logging() {
        let working_dir = WorkingDir::new(temp_dir());
        let spawner = ProcessSpawner::new(working_dir);

        // StdoutOnly
        let process = spawn_process_with_logging(&spawner, LoggingType::StdoutOnly);

        check_logs_settings(&process, LogSettingsQuery::Stdout, true).await;
        check_logs_settings(&process, LogSettingsQuery::Stderr, false).await;
        check_logs_settings(&process, LogSettingsQuery::Merged, false).await;

        // StderrOnly
        let process = spawn_process_with_logging(&spawner, LoggingType::StderrOnly);

        check_logs_settings(&process, LogSettingsQuery::Stdout, false).await;
        check_logs_settings(&process, LogSettingsQuery::Stderr, true).await;
        check_logs_settings(&process, LogSettingsQuery::Merged, false).await;

        // StdoutAndStderr
        let process = spawn_process_with_logging(&spawner, LoggingType::StdoutAndStderr);

        check_logs_settings(&process, LogSettingsQuery::Stdout, true).await;
        check_logs_settings(&process, LogSettingsQuery::Stderr, true).await;
        check_logs_settings(&process, LogSettingsQuery::Merged, false).await;

        // StdoutAndStderrMerged
        let process = spawn_process_with_logging(&spawner, LoggingType::StdoutAndStderrMerged);

        check_logs_settings(&process, LogSettingsQuery::Stdout, false).await;
        check_logs_settings(&process, LogSettingsQuery::Stderr, false).await;
        check_logs_settings(&process, LogSettingsQuery::Merged, true).await;
    }

    fn spawn_process_with_logging(spawner: &ProcessSpawner, logging_type: LoggingType) -> Process {
        let id = ProcessId::random();
        let cmd = echo_cmd(logging_type);
        spawner.spawn_runnable(&id, Box::new(cmd)).unwrap()
    }

    async fn check_logs_settings(process: &Process, query: LogSettingsQuery, expected: bool) {
        assert_eq!(
            expected,
            process
                .log_reader
                .as_ref()
                .unwrap()
                .check_logs_settings(query)
                .await
                .unwrap()
        );
    }

    fn cat_cmd() -> Cmd {
        Cmd::new("cat")
    }

    fn echo_cmd(logging_type: LoggingType) -> Cmd {
        Cmd::with_args_and_options(
            "echo",
            ["-n", "message"],
            CmdOptions::with_logging(logging_type),
        )
    }
}
