use std::path::PathBuf;

use proc_heim::{Cmd, CmdOptions, MessagingType};

pub fn hanging_forever_cmd() -> Cmd {
    Cmd::with_args("tail", ["-f", "/dev/stdin"])
}

pub fn echo_cmd_with_options(msg: &str, options: CmdOptions) -> Cmd {
    Cmd::with_args_and_options("echo", [msg], options)
}

pub fn echo_to_stderr_cmd_with_options(msg: &str, options: CmdOptions) -> Cmd {
    let cmd = format!("echo -n '${msg}' >> /dev/stderr");
    Cmd::with_args_and_options("bash", ["-c".into(), cmd], options)
}

pub fn bash_script(script_path: PathBuf, options: CmdOptions, mut args: Vec<String>) -> Cmd {
    let mut merged_args = vec!["-C".into(), script_path.to_str().unwrap().into()];
    merged_args.append(&mut args);
    Cmd::with_args_and_options("bash", merged_args, options)
}

pub mod std_io {
    use super::*;

    pub fn echo_cmd(msg: &str) -> Cmd {
        echo_cmd_with_options(
            msg,
            CmdOptions::with_message_output(MessagingType::StandardIo),
        )
    }

    pub fn echo_daemon_script_path() -> PathBuf {
        scripts_std_io().join("echo_daemon.sh") // TODO: use as scripts?
    }

    pub fn echo_json_script() -> PathBuf {
        scripts_std_io().join("echo_json.sh")
    }

    pub fn echo_stderr_script() -> PathBuf {
        scripts_std_io().join("echo_stderr.sh")
    }

    pub fn echo_all_args_script_path() -> PathBuf {
        scripts_std_io().join("echo_all_args.sh")
    }
}

pub mod named_pipe {
    use super::*;

    pub fn echo_daemon_script_path() -> PathBuf {
        scripts_named_pipe().join("echo_daemon.sh")
    }

    pub fn echo_script_path() -> PathBuf {
        scripts_named_pipe().join("echo.sh")
    }

    pub fn echo_script(msg: &str) -> Cmd {
        bash_script(
            echo_script_path(),
            CmdOptions::named_pipe(),
            vec![msg.into()],
        )
    }

    pub fn echo_json_script() -> PathBuf {
        scripts_named_pipe().join("echo_json.sh")
    }
}

fn scripts_std_io() -> PathBuf {
    scripts_dir().join("std_io")
}

fn scripts_named_pipe() -> PathBuf {
    scripts_dir().join("named_pipe")
}

fn scripts_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts")
}
