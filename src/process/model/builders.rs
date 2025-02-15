use std::{collections::HashMap, path::PathBuf};

use super::{
    validate_stdout_config, BufferCapacity, Cmd, CmdOptions, CmdOptionsError, LoggingType,
    MessagingType, Script, ScriptingLanguage,
};

/// Builder for [`Cmd`].
#[derive(Clone, Default, Debug)]
pub struct CmdBuilder {
    cmd: Option<String>,
    args: Option<Vec<String>>,
    options: Option<CmdOptions>,
}

impl CmdBuilder {
    /// Create empty builder. Same as [`CmdBuilder::default()`].
    pub fn builder() -> Self {
        CmdBuilder::default()
    }

    /// Set command name.
    pub fn cmd<T>(&mut self, cmd: T) -> &mut Self
    where
        T: Into<String>,
    {
        self.cmd = Some(cmd.into());
        self
    }

    /// Set command arguments.
    pub fn args<I, T>(&mut self, args: I) -> &mut Self
    where
        I: IntoIterator<Item = T>,
        T: Into<String>,
    {
        self.args = Some(args.into_iter().map(Into::into).collect());
        self
    }

    /// Set command options.
    pub fn options<T>(&mut self, options: T) -> &mut Self
    where
        T: Into<CmdOptions>,
    {
        self.options = Some(options.into());
        self
    }

    /// Create a [`Cmd`] from this builder.
    pub fn build(&mut self) -> Result<Cmd, CmdBuilderError> {
        Ok(Cmd {
            cmd: match self.cmd.take() {
                Some(value) => value,
                None => {
                    return Err(CmdBuilderError::UninitializedCmdName);
                }
            },
            args: match self.args.take() {
                Some(value) => value,
                None => Default::default(),
            },
            options: match self.options.take() {
                Some(value) => value,
                None => Default::default(),
            },
        })
    }
}

/// Enum returned from [`CmdBuilder::build`] method.
#[derive(thiserror::Error, Debug)]
pub enum CmdBuilderError {
    /// The command name was not initialized.
    #[error("The command name was not initialized")]
    UninitializedCmdName,
}

// CmdOptions ---------------------------------

/// Builder for [`CmdOptions`].
#[derive(Debug, Clone, Default)]
pub struct CmdOptionsBuilder {
    current_dir: Option<PathBuf>,
    clear_envs: bool,
    envs: Option<HashMap<String, String>>,
    envs_to_remove: Option<Vec<String>>,
    output_buffer_capacity: BufferCapacity,
    message_input: Option<MessagingType>,
    message_output: Option<MessagingType>,
    logging_type: Option<LoggingType>,
}

impl CmdOptionsBuilder {
    /// Create empty builder. Same as [`CmdOptionsBuilder::default()`].
    pub fn builder() -> Self {
        CmdOptionsBuilder::default()
    }

    /// Set process current directory.
    pub fn current_dir<T>(&mut self, current_dir: T) -> &mut Self
    where
        T: Into<PathBuf>,
    {
        self.current_dir = Some(current_dir.into());
        self
    }

    /// Clear or retain environment variables inherited from a parent process.
    pub fn clear_inherited_envs(&mut self, clear_envs: bool) -> &mut Self {
        self.clear_envs = clear_envs;
        self
    }

    /// Set environment variables.
    pub fn envs<K, V, I>(&mut self, envs: I) -> &mut Self
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
        self
    }

    /// Remove specific environment variables inherited from a parent process.
    pub fn inherited_envs_to_remove<I, T>(&mut self, envs: I) -> &mut Self
    where
        I: IntoIterator<Item = T>,
        T: Into<String>,
    {
        self.envs_to_remove = Some(envs.into_iter().map(Into::into).collect());
        self
    }

    /// Set message output buffer capacity.
    pub fn output_buffer_capacity(&mut self, capacity: BufferCapacity) -> &mut Self {
        self.output_buffer_capacity = capacity;
        self
    }

    /// Set message input type.
    pub fn message_input(&mut self, messaging_type: MessagingType) -> &mut Self {
        self.message_input = Some(messaging_type);
        self
    }

    /// Set message output type.
    pub fn message_output(&mut self, messaging_type: MessagingType) -> &mut Self {
        self.message_output = Some(messaging_type);
        self
    }

    /// Set logging type.
    pub fn logging_type(&mut self, logging_type: LoggingType) -> &mut Self {
        self.logging_type = Some(logging_type);
        self
    }

    /// Create a [`CmdOptions`] from this builder.
    pub fn build(&mut self) -> Result<CmdOptions, CmdOptionsError> {
        self.validate()?;
        Ok(CmdOptions {
            current_dir: self.current_dir.take(),
            clear_envs: self.clear_envs,
            envs: self.envs.take().unwrap_or_default(),
            envs_to_remove: self.envs_to_remove.take().unwrap_or_default(),
            output_buffer_capacity: self.output_buffer_capacity,
            message_input: self.message_input.take(),
            message_output: self.message_output.take(),
            logging_type: self.logging_type.take(),
        })
    }

    fn validate(&self) -> Result<(), CmdOptionsError> {
        validate_stdout_config(self.message_output.as_ref(), self.logging_type.as_ref())
    }
}

// Script -------------------------------------

/// Builder for [`Script`].
#[derive(Clone, Default, Debug)]
pub struct ScriptBuilder {
    language: Option<ScriptingLanguage>,
    content: Option<String>,
    args: Option<Vec<String>>,
    options: Option<CmdOptions>,
}

impl ScriptBuilder {
    /// Create empty builder. Same as [`ScriptBuilder::default()`].
    pub fn builder() -> Self {
        ScriptBuilder::default()
    }

    /// Set scripting language.
    pub fn language<T>(&mut self, language: T) -> &mut Self
    where
        T: Into<ScriptingLanguage>,
    {
        self.language = Some(language.into());
        self
    }

    /// Set script content.
    pub fn content<S>(&mut self, content: S) -> &mut Self
    where
        S: Into<String>,
    {
        self.content = Some(content.into());
        self
    }

    /// Set script arguments.
    pub fn args<I, T>(&mut self, args: I) -> &mut Self
    where
        I: IntoIterator<Item = T>,
        T: Into<String>,
    {
        self.args = Some(args.into_iter().map(Into::into).collect());
        self
    }

    /// Set script options.
    pub fn options<T>(&mut self, options: T) -> &mut Self
    where
        T: Into<CmdOptions>,
    {
        self.options = Some(options.into());
        self
    }

    /// Create a [`Script`] from this builder.
    pub fn build(&mut self) -> Result<Script, ScriptBuilderError> {
        Ok(Script {
            lang: match self.language.take() {
                Some(value) => value,
                None => {
                    return Err(ScriptBuilderError::UninitializedScriptingLanguage);
                }
            },
            content: match self.content.take() {
                Some(value) => value,
                None => {
                    return Err(ScriptBuilderError::UninitializedScriptContent);
                }
            },
            args: match self.args.take() {
                Some(value) => value,
                None => Default::default(),
            },
            options: match self.options.take() {
                Some(value) => value,
                None => Default::default(),
            },
        })
    }
}

/// Enum returned from [`ScriptBuilder::build`] method.
#[derive(thiserror::Error, Debug)]
pub enum ScriptBuilderError {
    /// The scripting language was not initialized.
    #[error("The scripting language was not initialized")]
    UninitializedScriptingLanguage,
    /// The script content was not initialized.
    #[error("The script content was not initialized")]
    UninitializedScriptContent,
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, path::PathBuf, str::FromStr};

    use crate::process::{
        model::builders::{ScriptBuilder, ScriptBuilderError},
        BufferCapacity, Cmd, CmdOptions, LoggingType, MessagingType, Script, ScriptingLanguage,
    };

    use super::{CmdBuilder, CmdOptionsBuilder};

    #[test]
    fn should_build_cmd() {
        let options = CmdOptions::with_standard_io_messaging();
        let expected = Cmd::with_args_and_options("ls", ["-l"], options.clone());
        let actual = CmdBuilder::builder()
            .cmd("ls")
            .args(["-l"])
            .options(options)
            .build()
            .unwrap();
        assert_eq!(expected, actual);
    }

    #[test]
    fn should_build_cmd_without_optional_fields() {
        let expected = Cmd::new("echo");
        let actual = CmdBuilder::builder().cmd("echo").build().unwrap();
        assert_eq!(expected, actual);
    }

    #[test]
    fn should_return_err_when_cmd_name_was_not_provided() {
        let cmd = CmdBuilder::builder().build();
        assert!(cmd.is_err());
    }

    #[test]
    fn should_build_script() {
        let options = CmdOptions::with_logging(LoggingType::StdoutAndStderrMerged);
        let expected =
            Script::with_args_and_options(ScriptingLanguage::Bash, "ls", ["-l"], options.clone());
        let actual = ScriptBuilder::builder()
            .language(ScriptingLanguage::Bash)
            .content("ls")
            .args(["-l"])
            .options(options)
            .build()
            .unwrap();
        assert_eq!(expected, actual);
    }

    #[test]
    fn should_build_script_without_optional_fields() {
        let expected = Script::new(ScriptingLanguage::Bash, "ls");
        let actual = ScriptBuilder::builder()
            .language(ScriptingLanguage::Bash)
            .content("ls")
            .build()
            .unwrap();
        assert_eq!(expected, actual);
    }

    #[test]
    fn should_return_err_when_script_lang_was_not_provided() {
        let result = ScriptBuilder::builder().content("ls").build();
        assert!(matches!(
            result,
            Err(ScriptBuilderError::UninitializedScriptingLanguage)
        ));
    }

    #[test]
    fn should_return_err_when_script_content_was_not_provided() {
        let result = ScriptBuilder::builder()
            .language(ScriptingLanguage::Perl)
            .build();
        assert!(matches!(
            result,
            Err(ScriptBuilderError::UninitializedScriptContent)
        ));
    }

    #[test]
    fn should_build_options() {
        let current_dir = PathBuf::from_str("/some/path").unwrap();
        let clear_envs = true;

        let mut envs = HashMap::new();
        envs.insert("ENV1", "value1");

        let env_to_remove = "PATH";

        let capacity = BufferCapacity::try_from(24).unwrap();
        let message_input = MessagingType::StandardIo;
        let message_output = MessagingType::NamedPipe;
        let logging_type = LoggingType::StderrOnly;

        let mut expected = CmdOptions::default();
        expected.set_current_dir(current_dir.clone());
        expected.clear_inherited_envs(clear_envs);
        expected.set_envs(envs.clone());
        expected.remove_env(env_to_remove);
        expected.set_message_output_buffer_capacity(capacity);
        expected.set_message_input(message_input.clone());
        expected.set_message_output(message_output.clone()).unwrap();
        expected.set_logging_type(logging_type.clone()).unwrap();

        let actual = CmdOptionsBuilder::builder()
            .current_dir(current_dir)
            .clear_inherited_envs(clear_envs)
            .envs(envs)
            .inherited_envs_to_remove([env_to_remove])
            .output_buffer_capacity(capacity)
            .message_input(message_input)
            .message_output(message_output)
            .logging_type(logging_type)
            .build()
            .unwrap();
        assert_eq!(expected, actual);
    }

    #[test]
    fn should_build_options_with_default_values() {
        let options = CmdOptionsBuilder::builder().build().unwrap();
        assert_eq!(BufferCapacity::default(), options.output_buffer_capacity);
        assert!(!options.clear_envs);
    }

    #[test]
    fn should_return_err_when_options_are_invalid() {
        let mut builder = CmdOptionsBuilder::builder();
        builder.message_output(MessagingType::StandardIo);

        assert!(builder
            .clone()
            .logging_type(LoggingType::StderrOnly)
            .build()
            .is_ok());

        assert!(builder
            .clone()
            .logging_type(LoggingType::StdoutAndStderr)
            .build()
            .is_err());

        assert!(builder
            .clone()
            .logging_type(LoggingType::StdoutAndStderrMerged)
            .build()
            .is_err());

        assert!(builder
            .clone()
            .logging_type(LoggingType::StdoutOnly)
            .build()
            .is_err());
    }
}
