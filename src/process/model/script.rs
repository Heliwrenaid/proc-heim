use super::{Cmd, CmdOptions, Runnable};
#[cfg(feature = "builder")]
use derive_builder::Builder;
use std::path::Path;

/// Constant used as a placeholder for a script file path. See [`CustomScriptRunConfig`] docs.
pub const SCRIPT_FILE_PATH_PLACEHOLDER: &str = "@FILE_PATH";

/// Enum type representing a scripting language.
///
/// `ScriptLanguage` provides run configuration for 8 most popular scripting languages.
/// If you want to use other language, see [`ScriptLanguage::Other`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ScriptLanguage {
    // TODO: rename to ScriptingLanguage
    /// Executes script with `bash` command.
    Bash,
    /// Executes script with `python` command.
    Python,
    /// Executes script with `php -f` command.
    Php,
    /// Executes script with `node` command.
    JavaScript,
    /// Executes script with `perl` command.
    Perl,
    /// Executes script with `lua` command.
    Lua,
    /// Executes script with `ruby` command.
    Ruby,
    /// Executes script with `groovy` command.
    Groovy,
    /// Executes script with provided configuration. See [`CustomScriptRunConfig`] docs.
    Other(CustomScriptRunConfig),
}

impl ScriptLanguage {
    fn get_cmd_and_args(&self, file_path: String) -> (&str, Vec<String>) {
        // TODO: refactor and return run config...
        match &self {
            ScriptLanguage::Bash => ("bash", vec![file_path]),
            ScriptLanguage::Python => ("python", vec![file_path]),
            ScriptLanguage::Php => ("php", vec!["-f".into(), file_path]),
            ScriptLanguage::JavaScript => ("node", vec![file_path]),
            ScriptLanguage::Perl => ("perl", vec![file_path]),
            ScriptLanguage::Lua => ("lua", vec![file_path]),
            ScriptLanguage::Ruby => ("ruby", vec![file_path]),
            ScriptLanguage::Groovy => ("groovy", vec![file_path]),
            ScriptLanguage::Other(run_config) => run_config.get_cmd_and_args(&file_path),
        }
    }

    fn get_file_extension(&self) -> &str {
        match &self {
            ScriptLanguage::Bash => "sh",
            ScriptLanguage::Python => "py",
            ScriptLanguage::Php => "php",
            ScriptLanguage::JavaScript => "js",
            ScriptLanguage::Perl => "pl",
            ScriptLanguage::Lua => "lua",
            ScriptLanguage::Ruby => "rb",
            ScriptLanguage::Groovy => "groovy",
            ScriptLanguage::Other(run_config) => &run_config.file_extension,
        }
    }
}

/// `CustomScriptRunConfig` allows to define own configuration used to run a script.
///
/// It describes command name, its arguments needed to run a script and also
/// a file extension typical for a given scripting language.
/// # Examples
/// Run configuration for PHP language (equivalent to [`ScriptLanguage::Php`]):
/// ```
/// use proc_heim::model::script::CustomScriptRunConfig;
/// use proc_heim::model::script::SCRIPT_FILE_PATH_PLACEHOLDER;
///
/// CustomScriptRunConfig::new("php", ["-f", SCRIPT_FILE_PATH_PLACEHOLDER], "php");
///
/// ```
/// [`SCRIPT_FILE_PATH_PLACEHOLDER`] constant is used to mark that in this argument should be a path to a script file.
/// Before spawning a script, the placeholder will be replaced by proper file path to the script (with extension provided in `file_extension` argument).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomScriptRunConfig {
    //TODO: rename to ScriptRunConfig
    cmd: String,
    args: Vec<String>,
    file_extension: String,
}

impl CustomScriptRunConfig {
    /// Creates a new run configuration.
    pub fn new<C, T, I, F>(cmd: C, args: I, file_extension: F) -> Self
    where
        C: Into<String>,
        T: Into<String>,
        I: IntoIterator<Item = T>,
        F: Into<String>,
    {
        Self {
            cmd: cmd.into(),
            args: args.into_iter().map(Into::into).collect(),
            file_extension: file_extension.into(),
        }
    }

    pub(crate) fn get_cmd_and_args(&self, file_path: &str) -> (&str, Vec<String>) {
        let args = self
            .args
            .iter()
            .map(|arg| {
                if arg == SCRIPT_FILE_PATH_PLACEHOLDER {
                    file_path
                } else {
                    arg
                }
                .to_owned()
            })
            .collect();
        (&self.cmd, args)
    }
}

/// `Script` represents a single script.
///
/// It requires at least to set a scripting language and content. Script's arguments and options are optional.
/// [`ScriptLanguage`] defines the language in which the script is implemented.
/// Currently, library supports 8 most popular scripting languages, but it is possible to support a custom ones via [`ScriptLanguage::Other`].
///
/// `Script` stores its content in a file and then executes [`Cmd`](struct@crate::model::command::Cmd) provided by [`Runnable`](trait@crate::model::Runnable) trait implementation.
#[cfg(not(feature = "builder"))]
#[derive(Debug, Clone)]
pub struct Script {
    pub(crate) lang: ScriptLanguage,
    pub(crate) content: String,
    pub(crate) args: Option<Vec<String>>,
    pub(crate) options: CmdOptions,
}

/// `Script` represents a single script.
///
/// It requires at least to set a scripting language and content. Script's arguments and options are optional.
/// [`ScriptLanguage`] defines the language in which the script is implemented.
/// Currently, library supports 8 most popular scripting languages, but it is possible to support a custom ones via [`ScriptLanguage::Other`].
///
/// `Script` stores its content in a file and then executes [`Cmd`](struct@crate::model::command::Cmd) provided by [`Runnable`](trait@crate::model::Runnable) trait implementation.
#[cfg(feature = "builder")]
#[derive(Debug, Clone, Builder, PartialEq, Eq)]
pub struct Script {
    #[builder(setter(into))]
    pub(crate) lang: ScriptLanguage,
    #[builder(setter(into))]
    pub(crate) content: String,
    #[builder(setter(into, strip_option), default)]
    pub(crate) args: Option<Vec<String>>,
    #[builder(setter(into), default)]
    pub(crate) options: CmdOptions,
}

impl Script {
    /// Creates a new script with given scripting language and content.
    /// # Examples
    /// ```
    /// # use proc_heim::model::script::*;
    /// Script::new(ScriptLanguage::Bash, r#"
    ///     user=$(echo $USER)
    ///     echo "Hello $user"
    /// "#);
    /// ```
    pub fn new<S>(lang: ScriptLanguage, content: S) -> Self
    where
        S: Into<String>,
    {
        Self {
            lang,
            content: content.into(),
            args: None,
            options: CmdOptions::default(),
        }
    }

    /// Creates a new script with given scripting language, content and arguments.
    /// # Examples
    /// ```
    /// # use proc_heim::model::script::*;
    /// Script::with_args(ScriptLanguage::Bash, "echo $@ | cut -d ' ' -f2", ["arg1", "arg2"]);
    /// ```
    pub fn with_args<S, T, I>(lang: ScriptLanguage, content: S, args: I) -> Self
    where
        S: Into<String>,
        T: Into<String>,
        I: IntoIterator<Item = T>,
    {
        Self {
            lang,
            content: content.into(),
            args: Some(args.into_iter().map(Into::into).collect()),
            options: CmdOptions::default(),
        }
    }

    /// Creates a new script with given scripting language, content and options.
    /// # Examples
    /// ```
    /// # use proc_heim::model::script::*;
    /// # use proc_heim::model::command::*;
    /// let content = r#"
    ///     for dir in "$(ls -d */)"; do
    ///        echo "$dir"
    ///     done
    ///"#;
    /// let options = CmdOptions::with_logging(LoggingType::StdoutOnly);
    /// Script::with_options(ScriptLanguage::Bash, content, options);
    /// ```
    pub fn with_options<S>(lang: ScriptLanguage, content: S, options: CmdOptions) -> Self
    where
        S: Into<String>,
    {
        Self {
            lang,
            content: content.into(),
            args: None,
            options,
        }
    }

    /// Creates a new script with given scripting language, content, arguments and options.
    /// # Examples
    /// ```
    /// # use proc_heim::model::script::*;
    /// # use proc_heim::model::command::*;
    /// let content = r#"
    ///     base_dir="$1"
    ///     for dir in "$(ls -d $base_dir/*/)"; do
    ///         echo "$dir"
    ///     done
    /// "#;
    /// let args = vec!["/some/path"];
    /// let options = CmdOptions::with_logging(LoggingType::StdoutOnly);
    /// Script::with_args_and_options(ScriptLanguage::Bash, content, args, options);
    /// ```
    pub fn with_args_and_options<S, T, I>(
        lang: ScriptLanguage,
        content: S,
        args: I,
        options: CmdOptions,
    ) -> Self
    where
        S: Into<String>,
        T: Into<String>,
        I: IntoIterator<Item = T>,
    {
        Self {
            lang,
            content: content.into(),
            args: Some(args.into_iter().map(Into::into).collect()),
            options,
        }
    }

    /// Set a script arguments.
    /// # Examples
    /// ```
    /// # use proc_heim::model::script::*;
    /// let mut script = Script::new(ScriptLanguage::Bash, "echo $@ | cut -d ' ' -f2");
    /// script.set_args(["arg1", "arg2"]);
    /// ```
    pub fn set_args<S, I>(&mut self, args: I)
    where
        S: Into<String>,
        I: IntoIterator<Item = S>,
    {
        self.args = Some(args.into_iter().map(Into::into).collect());
    }

    /// Set a script options.
    /// # Examples
    /// ```
    /// # use proc_heim::model::script::*;
    /// # use proc_heim::model::command::*;
    /// let mut script = Script::new(ScriptLanguage::Bash, "echo $@ | cut -d ' ' -f2");
    /// script.set_options(CmdOptions::with_standard_io_messaging());
    /// ```
    pub fn set_options(&mut self, options: CmdOptions) {
        self.options = options;
    }

    /// Add a new argument to the end of argument list.
    /// If arguments was not specified during `Script` creation, it will create new argument list with given argument.
    /// # Examples
    /// ```
    /// # use proc_heim::model::script::*;
    /// # use proc_heim::model::command::*;
    /// let mut script = Script::new(ScriptLanguage::Bash, "echo $@ | cut -d ' ' -f2");
    /// script.add_arg("arg1");
    /// script.add_arg("arg2");
    /// ```
    pub fn add_arg<S>(&mut self, arg: S)
    where
        S: Into<String>,
    {
        self.args.get_or_insert(Vec::new()).push(arg.into());
    }

    /// Update script options via mutable reference.
    /// # Examples
    /// ```
    /// # use proc_heim::model::command::*;
    /// # use proc_heim::model::script::*;
    /// let mut script = Script::new(ScriptLanguage::Bash, "echo $TEST_ENV_VAR | cut -d ' ' -f2");
    /// script.options_mut().add_env("TEST_ENV_VAR", "example value");
    /// ```
    pub fn options_mut(&mut self) -> &mut CmdOptions {
        &mut self.options
    }
}

impl Runnable for Script {
    fn bootstrap_cmd(&self, process_dir: &Path) -> Result<Cmd, String> {
        let file_path = create_script_file(self, process_dir)?;
        let (cmd, mut args) = self.lang.get_cmd_and_args(file_path);

        if let Some(arguments) = &self.args {
            args.extend_from_slice(arguments);
        }

        let cmd = Cmd {
            cmd: cmd.into(),
            args: args.into(),
            options: self.options.clone(),
        };
        Ok(cmd)
    }
}

fn create_script_file(script: &Script, script_file_dir: &Path) -> Result<String, String> {
    let extension = script.lang.get_file_extension();
    let file_path = script_file_dir.join("script").with_extension(extension);
    std::fs::write(&file_path, &script.content).map_err(|err| err.to_string())?;
    file_path
        .to_str()
        .ok_or("Script file path cannot be converted to UTF-8 string".to_owned())
        .map(|v| v.to_owned())
}
