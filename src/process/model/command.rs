use std::{collections::HashMap, path::PathBuf};

use derive_builder::Builder;

#[derive(Debug, Clone, Builder)]
pub struct Cmd {
    pub cmd: String,
    #[builder(default = "None")]
    pub args: Option<Vec<String>>,
    #[builder(default)]
    pub options: CmdOptions,
}

#[derive(Debug, Clone, Builder, Default)]
pub struct CmdOptions {
    #[builder(default = "None")]
    pub current_dir: Option<PathBuf>,
    #[builder(default = "false")]
    pub clear_envs: bool,
    #[builder(default = "None")]
    pub envs: Option<HashMap<String, String>>,
    #[builder(default = "None")]
    pub output_buffer_capacity: Option<usize>,
    #[builder(default = "None")]
    pub message_input: Option<MessagingType>,
    #[builder(default = "None")]
    pub message_output: Option<MessagingType>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessagingType {
    StandardIo,
    NamedPipe,
}

impl CmdOptions {
    pub fn std_io() -> CmdOptions {
        Self::with_same_in_out(MessagingType::StandardIo)
    }

    pub fn named_pipe() -> CmdOptions {
        Self::with_same_in_out(MessagingType::NamedPipe)
    }

    fn with_same_in_out(messaging_type: MessagingType) -> CmdOptions {
        CmdOptionsBuilder::default()
            .message_input(Some(messaging_type.clone()))
            .message_output(Some(messaging_type))
            .build()
            .unwrap()
    }
}
