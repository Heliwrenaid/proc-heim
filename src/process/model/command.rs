use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use crate::Runnable;

#[cfg(not(feature = "builder"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cmd {
    pub(crate) cmd: String,
    pub(crate) args: Option<Vec<String>>,
    pub(crate) options: CmdOptions,
}

impl Cmd {
    pub fn new<S>(cmd: S) -> Self
    where
        S: Into<String>,
    {
        Self {
            cmd: cmd.into(),
            args: None,
            options: CmdOptions::default(),
        }
    }

    pub fn with_args<S, T, I>(cmd: S, args: I) -> Self
    where
        S: Into<String>,
        T: Into<String>,
        I: IntoIterator<Item = T>,
    {
        Self {
            cmd: cmd.into(),
            args: Some(args.into_iter().map(Into::into).collect()),
            options: CmdOptions::default(),
        }
    }

    pub fn with_options<S>(cmd: S, options: CmdOptions) -> Self
    where
        S: Into<String>,
    {
        Self {
            cmd: cmd.into(),
            args: None,
            options,
        }
    }

    pub fn with_args_and_options<S, T, I>(cmd: S, args: I, options: CmdOptions) -> Self
    where
        S: Into<String>,
        T: Into<String>,
        I: IntoIterator<Item = T>,
    {
        Self {
            cmd: cmd.into(),
            args: Some(args.into_iter().map(Into::into).collect()),
            options,
        }
    }

    pub fn set_args<S, I>(&mut self, args: I)
    where
        S: Into<String>,
        I: IntoIterator<Item = S>,
    {
        self.args = Some(args.into_iter().map(Into::into).collect());
    }

    pub fn set_options(&mut self, options: CmdOptions) {
        self.options = options;
    }

    pub fn add_arg<S>(&mut self, arg: S)
    where
        S: Into<String>,
    {
        self.args.get_or_insert(Vec::new()).push(arg.into());
    }

    pub fn options_mut(&mut self) -> &mut CmdOptions {
        &mut self.options
    }
}

#[cfg(not(feature = "builder"))]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CmdOptions {
    pub(crate) current_dir: Option<PathBuf>,
    pub(crate) clear_envs: bool,
    pub(crate) envs: Option<HashMap<String, String>>,
    pub(crate) envs_to_remove: Option<Vec<String>>,
    pub(crate) output_buffer_capacity: Option<usize>,
    pub(crate) message_input: Option<MessagingType>,
    pub(crate) message_output: Option<MessagingType>,
    pub(crate) logging_type: Option<LoggingType>,
}

impl CmdOptions {
    pub fn standard_io() -> CmdOptions {
        Self::with_same_in_out(MessagingType::StandardIo)
    }

    pub fn named_pipe() -> CmdOptions {
        Self::with_same_in_out(MessagingType::NamedPipe)
    }

    fn with_same_in_out(messaging_type: MessagingType) -> CmdOptions {
        CmdOptions {
            message_input: messaging_type.clone().into(),
            message_output: messaging_type.into(),
            ..Default::default()
        }
    }

    pub fn with_message_input(message_input: MessagingType) -> Self {
        Self {
            message_input: message_input.into(),
            ..Default::default()
        }
    }

    pub fn with_message_output(message_output: MessagingType) -> Self {
        Self {
            message_output: message_output.into(),
            ..Default::default()
        }
    }

    pub fn with_logging(logging_type: LoggingType) -> Self {
        Self {
            logging_type: logging_type.into(),
            ..Default::default()
        }
    }

    pub fn set_current_dir(&mut self, dir: PathBuf) {
        self.current_dir = dir.into();
    }

    pub fn set_clear_envs(&mut self, value: bool) {
        self.clear_envs = value;
    }

    pub fn set_envs<K, V, I>(&mut self, envs: I)
    where
        K: Into<String>,
        V: Into<String>,
        I: IntoIterator<Item = (K, V)>,
    {
        self.envs = Some(
            envs.into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
        );
    }

    pub fn add_env<K, V>(&mut self, name: K, value: V)
    where
        K: Into<String>,
        V: Into<String>,
    {
        self.envs
            .get_or_insert(HashMap::new())
            .insert(name.into(), value.into());
    }

    pub fn remove_env<S>(&mut self, name: S)
    where
        S: Into<String> + AsRef<str>,
    {
        if let Some(envs) = self.envs.as_mut() {
            envs.remove(name.as_ref());
        }
        self.envs_to_remove
            .get_or_insert(Vec::new())
            .push(name.into());
    }

    pub fn set_message_input(&mut self, messaging_type: MessagingType) {
        self.message_input = messaging_type.into();
    }

    pub fn set_message_output(
        &mut self,
        messaging_type: MessagingType,
    ) -> Result<(), CmdOptionsError> {
        validate_stdout_config(Some(&messaging_type), self.logging_type.as_ref())?;
        self.message_output = messaging_type.into();
        Ok(())
    }

    pub fn set_logging_type(&mut self, logging_type: LoggingType) -> Result<(), CmdOptionsError> {
        validate_stdout_config(self.message_output.as_ref(), Some(&logging_type))?;
        self.logging_type = logging_type.into();
        Ok(())
    }

    pub fn set_message_output_buffer_capacity(&mut self, capacity: usize) {
        self.output_buffer_capacity = capacity.into();
    }
}

fn validate_stdout_config(
    messaging_type: Option<&MessagingType>,
    logging_type: Option<&LoggingType>,
) -> Result<(), CmdOptionsError> {
    if let (Some(messaging_type), Some(logging_type)) = (messaging_type, logging_type) {
        if messaging_type == &MessagingType::StandardIo && logging_type != &LoggingType::StderrOnly
        {
            return Err(CmdOptionsError::StdoutConfigurationConflict(
                messaging_type.to_owned(),
                logging_type.to_owned(),
            ));
        }
    }
    Ok(())
}

#[derive(thiserror::Error, Debug)]
pub enum CmdOptionsError {
    #[error("Cannot use {0:?} together with {1:?} for stdout configuration")]
    StdoutConfigurationConflict(MessagingType, LoggingType),
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

impl Runnable for Cmd {
    fn bootstrap_cmd(&self, _process_dir: &Path) -> Result<Cmd, String> {
        Ok(self.clone())
    }
}

#[cfg(feature = "builder")]
pub use builder::*;

#[cfg(feature = "builder")]
pub mod builder {
    use super::*;
    use derive_builder::Builder;

    #[derive(Debug, Clone, Builder, PartialEq, Eq)]
    pub struct Cmd {
        #[builder(setter(into))]
        pub(crate) cmd: String,
        #[builder(setter(into, strip_option), default)]
        pub(crate) args: Option<Vec<String>>,
        #[builder(setter(into), default)]
        pub(crate) options: CmdOptions,
    }

    #[derive(Debug, Clone, Default, Builder, PartialEq, Eq)]
    #[builder(build_fn(validate = "Self::validate"))]
    pub struct CmdOptions {
        #[builder(setter(into, strip_option), default)]
        pub(crate) current_dir: Option<PathBuf>,
        #[builder(setter(into, strip_option), default = "false")]
        pub(crate) clear_envs: bool,
        #[builder(setter(into, strip_option), default)]
        pub(crate) envs: Option<HashMap<String, String>>,
        #[builder(setter(into, strip_option), default)]
        pub(crate) envs_to_remove: Option<Vec<String>>,
        #[builder(setter(into, strip_option), default)]
        pub(crate) output_buffer_capacity: Option<usize>,
        #[builder(setter(into, strip_option), default)]
        pub(crate) message_input: Option<MessagingType>,
        #[builder(setter(into, strip_option), default)]
        pub(crate) message_output: Option<MessagingType>,
        #[builder(setter(into, strip_option), default)]
        pub(crate) logging_type: Option<LoggingType>,
    }

    impl CmdOptionsBuilder {
        fn validate(&self) -> Result<(), String> {
            if let (Some(message_output), Some(logging_type)) =
                (self.message_output.as_ref(), self.logging_type.as_ref())
            {
                validate_stdout_config(message_output.as_ref(), logging_type.as_ref())
                    .map_err(|err| err.to_string())?;
            }
            Ok(())
        }
    }
}
