use std::path::PathBuf;

use proc_heim::{Cmd, CmdBuilder, CmdOptions, CmdOptionsBuilder, MessagingType};

pub fn cat_cmd() -> Cmd {
    CmdBuilder::default().cmd("cat".into()).build().unwrap()
}

pub fn bash_script(script_path: PathBuf, options: CmdOptions, mut args: Vec<String>) -> Cmd {
    let mut merged_args = vec!["-C".into(), script_path.to_str().unwrap().into()];
    merged_args.append(&mut args);

    CmdBuilder::default()
        .cmd("bash".into())
        .args(merged_args.into())
        .options(options)
        .build()
        .unwrap()
}

pub mod std_io {
    use super::*;

    pub fn echo_cmd(msg: &str) -> Cmd {
        CmdBuilder::default()
            .cmd("echo".into())
            .args(vec![msg.into()].into())
            .options(
                CmdOptionsBuilder::default()
                    .message_output(Some(MessagingType::StandardIo))
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap()
    }

    pub fn echo_daemon_script_path() -> PathBuf {
        scripts_std_io().join("echo_daemon.sh")
    }

    pub fn echo_json_script() -> PathBuf {
        scripts_std_io().join("echo_json.sh")
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
