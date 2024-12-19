use std::{collections::HashMap, path::PathBuf};

use derive_builder::Builder;

use crate::Runnable;

#[derive(Debug, Clone, Builder)]
pub struct Cmd {
    #[builder(setter(into))]
    pub cmd: String,
    #[builder(setter(into, strip_option), default)]
    pub args: Option<Vec<String>>,
    #[builder(setter(into), default)]
    pub options: CmdOptions,
}

#[derive(Debug, Clone, Builder, Default)]
#[builder(build_fn(validate = "Self::validate"))]
pub struct CmdOptions {
    #[builder(setter(into, strip_option), default)]
    pub current_dir: Option<PathBuf>,
    #[builder(setter(into, strip_option), default = "false")]
    pub clear_envs: bool,
    #[builder(setter(into, strip_option), default)]
    pub envs: Option<HashMap<String, String>>,
    #[builder(setter(into, strip_option), default)]
    pub output_buffer_capacity: Option<usize>,
    #[builder(setter(into, strip_option), default)]
    pub message_input: Option<MessagingType>,
    #[builder(setter(into, strip_option), default)]
    pub message_output: Option<MessagingType>,
    #[builder(setter(into, strip_option), default)]
    pub logging_type: Option<LoggingType>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessagingType {
    StandardIo,
    NamedPipe,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoggingType {
    StdoutOnly,
    StderrOnly,
    StdoutAndStderr,
    StdoutAndStderrMerged,
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
            .message_input(messaging_type.clone())
            .message_output(messaging_type)
            .build()
            .unwrap()
    }
}

impl CmdOptionsBuilder {
    fn validate(&self) -> Result<(), String> {
        if let Some(ref message_output) = self.message_output {
            if let Some(message_output) = message_output {
                if message_output == &MessagingType::StandardIo && self.logging_type.is_some() {
                    if let Some(logging_type) = self.logging_type.as_ref().unwrap() {
                        if logging_type != &LoggingType::StderrOnly {
                            Err(format!(
                                "Cannot setup logging type: {:?}, when message output is: {:?}",
                                logging_type, message_output
                            ))?;
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

impl Runnable for Cmd {
    fn bootstrap_cmd(&self, _process_dir: &PathBuf) -> Result<Cmd, String> {
        Ok(self.clone())
    }
}
