//! [`CmdOptionsError::StdoutConfigurationConflict`]: crate::model::command::CmdOptionsError::StdoutConfigurationConflict
//! [`MessagingType::StandardIo`]: crate::model::command::MessagingType::StandardIo
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use super::Runnable;
// TODO: make BashShell wrapper (bash -c ...) ?
/// `Cmd` represents a single command.
///
/// It requires at least to set a command name.
/// Command's arguments and options are optional.
///
/// Note that using input/output redirection symbols (eg. `|`, `>>`, `2>`) as command arguments will fail.
/// Instead use [`Script`](struct@crate::model::script::Script).
#[cfg(not(feature = "builder"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cmd {
    pub(crate) cmd: String,
    pub(crate) args: Option<Vec<String>>,
    pub(crate) options: CmdOptions,
}

// TODO: add parse() and parse_with_options()
impl Cmd {
    /// Creates a new command with given name.
    /// # Examples
    /// ```
    /// # use proc_heim::model::command::Cmd;
    /// Cmd::new("echo");
    /// ```
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

    /// Creates a new command with given name and arguments.
    /// # Examples
    /// ```
    /// # use proc_heim::model::command::Cmd;
    /// Cmd::with_args("ls", ["-l", "~"]);
    /// ```
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

    /// Creates a new command with given name and options.
    /// # Examples
    /// ```
    /// # use proc_heim::model::command::*;
    /// Cmd::with_options("ls", CmdOptions::default());
    /// ```
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

    /// Creates a new command with given name, arguments and options.
    /// # Examples
    /// ```
    /// # use proc_heim::model::command::*;
    /// Cmd::with_args_and_options("ls", ["-l"], CmdOptions::default());
    /// ```
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

    /// Set a command arguments.
    /// # Examples
    /// ```
    /// # use proc_heim::model::command::*;
    /// let mut cmd = Cmd::new("ls");
    /// cmd.set_args(["-la", "~"]);
    /// ```
    pub fn set_args<S, I>(&mut self, args: I)
    where
        S: Into<String>,
        I: IntoIterator<Item = S>,
    {
        self.args = Some(args.into_iter().map(Into::into).collect());
    }

    /// Set a command options.
    /// # Examples
    /// ```
    /// # use proc_heim::model::command::*;
    /// let mut cmd = Cmd::new("ls");
    /// cmd.set_options(CmdOptions::default());
    /// ```
    pub fn set_options(&mut self, options: CmdOptions) {
        self.options = options;
    }

    /// Add a new argument to the end of argument list.
    /// If arguments was not specified during `Cmd` creation, it will create new argument list with given argument.
    /// # Examples
    /// ```
    /// # use proc_heim::model::command::*;
    /// let mut cmd = Cmd::new("ls");
    /// cmd.add_arg("-l");
    /// cmd.add_arg("/some/directory");
    /// ```
    pub fn add_arg<S>(&mut self, arg: S)
    where
        S: Into<String>,
    {
        self.args.get_or_insert(Vec::new()).push(arg.into());
    }

    /// Update command options via mutable reference.
    /// # Examples
    /// ```
    /// # use proc_heim::model::command::*;
    /// let mut cmd = Cmd::new("env");
    /// cmd.options_mut().add_env("TEST_ENV_VAR", "value");
    /// ```
    pub fn options_mut(&mut self) -> &mut CmdOptions {
        &mut self.options
    }
}

/// Wrapper type used to define buffer capacity.
///
/// Capacity must be greater than 0 and less or equal `usize::MAX / 2`.
/// # Examples
/// ```
/// # use proc_heim::model::command::*;
/// let capacity = BufferCapacity::try_from(16).unwrap();
/// assert_eq!(16, *capacity.as_ref());
/// ```
/// ```
/// # use proc_heim::model::command::*;
/// let result = BufferCapacity::try_from(0);
/// assert!(result.is_err());
/// ```
/// ```
/// # use proc_heim::model::command::*;
/// let result = BufferCapacity::try_from(usize::MAX / 2 + 1);
/// assert!(result.is_err());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct BufferCapacity {
    pub(crate) inner: usize,
}

impl TryFrom<usize> for BufferCapacity {
    type Error = String;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        if value == 0 || value > usize::MAX / 2 {
            Err("Buffer capacity must be greater than 0 and less or equal usize::MAX / 2".into())
        } else {
            Ok(Self { inner: value })
        }
    }
}

impl AsRef<usize> for BufferCapacity {
    fn as_ref(&self) -> &usize {
        &self.inner
    }
}

/// Default capacity value is 16.
impl Default for BufferCapacity {
    fn default() -> Self {
        Self { inner: 16 }
    }
}

/// `CmdOptions` are used to describe command's additional settings.
///
/// It allows to configure command's input/outputs, working_directory and environment variables.
///
/// Command's input allows to send messages from parent process, and receive them in spawned (child) process.
/// Whereas the message output of the command is used for communication in the opposite direction.
/// Communication with a process can be done using standard I/O or named pipes.
///
/// It is also possible to set logging in order to allow child process to produce logs
/// which, unlike messages, are stored permanently and therefore can be read multiple times by parent process.
#[cfg(not(feature = "builder"))]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CmdOptions {
    pub(crate) current_dir: Option<PathBuf>,
    pub(crate) clear_envs: bool,
    pub(crate) envs: Option<HashMap<String, String>>,
    pub(crate) envs_to_remove: Option<Vec<String>>,
    pub(crate) output_buffer_capacity: BufferCapacity,
    pub(crate) message_input: Option<MessagingType>,
    pub(crate) message_output: Option<MessagingType>,
    pub(crate) logging_type: Option<LoggingType>,
}

impl CmdOptions {
    /// Create options with configured messaging input/output via standard I/O.
    pub fn with_standard_io_messaging() -> CmdOptions {
        Self::with_same_in_out(MessagingType::StandardIo)
    }

    /// Create options with configured messaging input/output via named pipes.
    pub fn with_named_pipe_messaging() -> CmdOptions {
        Self::with_same_in_out(MessagingType::NamedPipe)
    }

    fn with_same_in_out(messaging_type: MessagingType) -> CmdOptions {
        CmdOptions {
            message_input: messaging_type.clone().into(),
            message_output: messaging_type.into(),
            ..Default::default()
        }
    }

    /// Create options with configured messaging input type.
    pub fn with_message_input(message_input: MessagingType) -> Self {
        Self {
            message_input: message_input.into(),
            ..Default::default()
        }
    }

    /// Create options with configured messaging output type.
    pub fn with_message_output(message_output: MessagingType) -> Self {
        Self {
            message_output: message_output.into(),
            ..Default::default()
        }
    }

    /// Create options with configured logging type.
    pub fn with_logging(logging_type: LoggingType) -> Self {
        Self {
            logging_type: logging_type.into(),
            ..Default::default()
        }
    }

    /// Set process's working directory.
    pub fn set_current_dir(&mut self, dir: PathBuf) {
        self.current_dir = dir.into();
    }

    /// By default, child process will inherit all environment variables from the parent.
    /// To prevent this behavior set this value to `true`.
    pub fn clear_inherited_envs(&mut self, value: bool) {
        self.clear_envs = value;
    }

    /// Set environment variables for a process.
    /// # Examples
    /// ```
    /// # use proc_heim::model::command::*;
    /// # use std::collections::HashMap;
    /// let mut envs = HashMap::new();
    /// envs.insert("TEST_ENV_VAR_1", "value1");
    /// envs.insert("TEST_ENV_VAR_2", "value2");
    ///
    /// let mut options = CmdOptions::default();
    /// options.set_envs(envs);
    /// ```
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

    /// Add or update single environment variable.
    pub fn add_env<K, V>(&mut self, name: K, value: V)
    where
        K: Into<String>,
        V: Into<String>,
    {
        self.envs
            .get_or_insert(HashMap::new())
            .insert(name.into(), value.into());
    }

    /// Remove single environment variable (manually set earlier and also inherited from the parent process).
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

    /// Set message input type.
    pub fn set_message_input(&mut self, messaging_type: MessagingType) {
        self.message_input = messaging_type.into();
    }

    /// Set message output type.
    ///
    /// This method will return [`CmdOptionsError::StdoutConfigurationConflict`]
    /// when trying to set [`MessagingType::StandardIo`] and logging to stdout was previously configured.
    pub fn set_message_output(
        &mut self,
        messaging_type: MessagingType,
    ) -> Result<(), CmdOptionsError> {
        validate_stdout_config(Some(&messaging_type), self.logging_type.as_ref())?;
        self.message_output = messaging_type.into();
        Ok(())
    }

    /// Set logging type.
    ///
    /// This method will return [`CmdOptionsError::StdoutConfigurationConflict`]
    /// when trying to set logging to stdout and message output was previously configured as [`MessagingType::StandardIo`].
    pub fn set_logging_type(&mut self, logging_type: LoggingType) -> Result<(), CmdOptionsError> {
        validate_stdout_config(self.message_output.as_ref(), Some(&logging_type))?;
        self.logging_type = logging_type.into();
        Ok(())
    }

    /// Set message output buffer capacity for receiving end (parent process).
    ///
    /// When parent process is not reading messages produced by a child process,
    /// then the messages are buffered up to the given `capacity` value.
    /// If the buffer limit is reached and a child process sends a new message, the "oldest" buffered message will be removed.
    pub fn set_message_output_buffer_capacity(&mut self, capacity: BufferCapacity) {
        self.output_buffer_capacity = capacity;
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

/// Enum returned from fallible `CmdOptions` methods
#[derive(thiserror::Error, Debug)]
pub enum CmdOptionsError {
    /// Standard output can only be used for logging or messaging, but not both.
    /// When you need to use both functionalities, then configure message output as [`MessagingType::NamedPipe`].
    ///
    /// For more information, see [`CmdOptions::set_message_output`] or [`CmdOptions::set_logging_type`].
    #[error("Cannot use {0:?} together with {1:?} for stdout configuration")]
    StdoutConfigurationConflict(MessagingType, LoggingType),
}

/// Enum representing messaging type of a spawned process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessagingType {
    /// Communicate with a spawned process via standard I/O.
    StandardIo,
    /// Communicate with a spawned process via named pipes.
    NamedPipe,
}

/// Enum representing logging type of a spawned process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoggingType {
    /// Collect logs only from standard output stream.
    StdoutOnly,
    /// Collect logs only from standard error stream.
    StderrOnly,
    /// Collect logs from both: standard output and error streams.
    StdoutAndStderr,
    /// Collect logs from one stream, created by merged standard output and error streams.
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

    /// `Cmd` represents a single command.
    ///
    /// It requires at least to set a command name.
    /// Command's arguments and options are optional.
    ///
    /// Note that using input/output redirection symbols (eg. `|`, `>>`, `2>`) as command arguments will fail.
    /// Instead use [`Script`](struct@crate::model::script::Script).
    #[derive(Debug, Clone, Builder, PartialEq, Eq)]
    pub struct Cmd {
        #[builder(setter(into))]
        pub(crate) cmd: String,
        #[builder(setter(into, strip_option), default)]
        pub(crate) args: Option<Vec<String>>,
        #[builder(setter(into), default)]
        pub(crate) options: CmdOptions,
    }

    /// `CmdOptions` are used to describe command's additional settings.
    ///
    /// It allows to configure command's input/outputs, working_directory and environment variables.
    ///
    /// Command's input allows to send messages from parent process, and receive them in spawned (child) process.
    /// Whereas the message output of the command is used for communication in the opposite direction.
    /// Communication with a process can be done using standard I/O or named pipes.
    ///
    /// It is also possible to set logging in order to allow child process to produce logs
    /// which, unlike messages, are stored permanently and therefore can be read multiple times by parent process.
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
        pub(crate) output_buffer_capacity: BufferCapacity,
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
