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
}
