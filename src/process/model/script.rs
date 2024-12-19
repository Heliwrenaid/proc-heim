use std::path::PathBuf;

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
    pub fn new(cmd: &str, args: Vec<String>, file_extension: &str) -> Self {
        Self {
            cmd: cmd.into(),
            args,
            file_extension: file_extension.into(),
        }
    }

    pub fn get_cmd_and_args(&self, file_path: &str) -> (&str, Vec<String>) {
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
    pub lang: ScriptLanguage,
    #[builder(setter(into))]
    pub content: String,
    #[builder(setter(into, strip_option), default)]
    pub args: Option<Vec<String>>,
    #[builder(setter(into), default)]
    pub options: CmdOptions,
}

impl Runnable for Script {
    fn bootstrap_cmd(&self, process_dir: &PathBuf) -> Result<Cmd, String> {
        let file_path = create_script_file(&self, &process_dir)?;
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

fn create_script_file(script: &Script, script_file_dir: &PathBuf) -> Result<String, String> {
    let extension = script.lang.get_file_extension();
    let file_path = script_file_dir.join("script").with_extension(extension);
    std::fs::write(&file_path, &script.content).map_err(|err| err.to_string())?;
    file_path
        .to_str()
        .ok_or("Script file path cannot be converted to UTF-8 string".to_owned())
        .map(|v| v.to_owned())
}
