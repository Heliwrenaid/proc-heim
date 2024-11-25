use std::path::PathBuf;

use derive_builder::Builder;
use tokio::process::Child;

#[derive(Builder, Debug)]
#[builder(pattern = "owned")]
pub struct Process {
    pub child: Child,
    #[builder(default = "None")]
    pub input_pipe: Option<PathBuf>,
    #[builder(default = "None")]
    pub output_pipe: Option<PathBuf>,
}
