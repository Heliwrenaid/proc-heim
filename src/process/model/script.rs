use std::path::Path;

use derive_builder::Builder;

use crate::{Cmd, CmdOptions, Runnable};

pub const SCRIPT_FILE_PATH_PLACEHOLDER: &str = "$FILE_PATH";

#[derive(Debug, Clone)]
pub enum ScriptLanguage {
    Bash,
    Python,
    Php,
    JavaScript,
    Perl,
    Lua,
    Ruby,
    Groovy,
    Other(CustomScriptRunConfig),
}

impl ScriptLanguage {
    fn get_cmd_and_args(&self, file_path: String) -> (&str, Vec<String>) {
        match &self {
            ScriptLanguage::Bash => ("bash", vec!["-C".into(), file_path]),
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

#[derive(Debug, Clone)]
pub struct CustomScriptRunConfig {
    cmd: String,
    args: Vec<String>,
    file_extension: String,
}

impl CustomScriptRunConfig {
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

#[derive(Debug, Clone, Builder)]
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

    pub fn with_args<S, T, I>(lang: ScriptLanguage, content: S, args: I) -> Self
    where
        S: Into<String>,
        T: Into<String>,
        I: IntoIterator<Item = S>,
    {
        Self {
            lang,
            content: content.into(),
            args: Some(args.into_iter().map(Into::into).collect()),
            options: CmdOptions::default(),
        }
    }

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

    pub fn with_args_and_option<S, T, I>(
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

    pub fn get_mut_options(&mut self) -> &mut CmdOptions {
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
