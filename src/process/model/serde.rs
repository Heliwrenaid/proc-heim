use std::{collections::HashMap, path::PathBuf};

use serde::{de::Error, Deserialize, Deserializer};

use crate::process::{validate_stdout_config, LoggingType, MessagingType};

use super::{BufferCapacity, CmdOptions};

impl<'de> Deserialize<'de> for BufferCapacity {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct BufferCapacityHelper(usize);

        let helper = BufferCapacityHelper::deserialize(deserializer)?;
        BufferCapacity::try_from(helper.0).map_err(Error::custom)
    }
}

impl serde::Serialize for BufferCapacity {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_i64(self.inner as i64)
    }
}

impl<'de> Deserialize<'de> for CmdOptions {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize, Default)]
        #[serde(default)]
        struct CmdOptionsHelper {
            current_dir: Option<PathBuf>,
            clear_envs: bool,
            envs: Option<HashMap<String, String>>,
            envs_to_remove: Option<Vec<String>>,
            output_buffer_capacity: BufferCapacity,
            message_input: Option<MessagingType>,
            message_output: Option<MessagingType>,
            logging_type: Option<LoggingType>,
        }

        let mut helper = CmdOptionsHelper::deserialize(deserializer)?;

        validate_stdout_config(helper.message_output.as_ref(), helper.logging_type.as_ref())
            .map_err(Error::custom)?;

        if let (Some(envs), Some(envs_to_remove)) = (helper.envs.as_mut(), &helper.envs_to_remove) {
            for env in envs_to_remove {
                envs.remove(env);
            }
        }

        Ok(CmdOptions {
            current_dir: helper.current_dir,
            clear_envs: helper.clear_envs,
            envs: helper.envs,
            envs_to_remove: helper.envs_to_remove,
            output_buffer_capacity: helper.output_buffer_capacity,
            message_input: helper.message_input,
            message_output: helper.message_output,
            logging_type: helper.logging_type,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, path::PathBuf};

    use crate::process::{
        BufferCapacity, Cmd, CmdOptions, LoggingType, MessagingType, Script, ScriptRunConfig,
        ScriptingLanguage,
    };

    // BufferCapacity ----------------------------------------------

    #[test]
    fn should_serialize_buffer_capacity() {
        let capacity = BufferCapacity::try_from(123).unwrap();

        let serialized = serde_json::to_string(&capacity).unwrap();
        assert_eq!("123", serialized);
    }

    #[test]
    fn should_deserialize_buffer_capacity() {
        let expected = BufferCapacity::try_from(9876).unwrap();

        let deserialized = serde_json::from_str("9876").unwrap();
        assert_eq!(expected, deserialized);
    }

    #[test]
    fn should_return_err_when_deserializing_invalid_buffer_capacity() {
        let value = usize::MAX / 2 + 1;
        let result = serde_json::from_str::<'_, BufferCapacity>(&value.to_string());
        assert!(result.is_err());
        let expected_msg =
            "Buffer capacity must be greater than 0 and less or equal usize::MAX / 2";
        assert!(result.err().unwrap().to_string().contains(expected_msg));

        let value = 0;
        let result = serde_json::from_str::<'_, BufferCapacity>(&value.to_string());
        assert!(result.is_err());
        assert!(result.err().unwrap().to_string().contains(expected_msg));

        let value = -1;
        let result = serde_json::from_str::<'_, BufferCapacity>(&value.to_string());
        assert!(result.is_err());

        let value = 3.4;
        let result = serde_json::from_str::<'_, BufferCapacity>(&value.to_string());
        assert!(result.is_err());
    }

    // CmdOptions ----------------------------------------------

    #[test]
    fn should_serialize_and_deserialize_options() {
        let options = create_options();

        let serialized = serde_json::to_string(&options).unwrap();
        assert_eq!(options_json(), serialized);

        let deserialized = serde_json::from_str(&serialized).unwrap();
        assert_eq!(options, deserialized);
    }

    #[test]
    fn should_deserialize_options_with_defaults() {
        // without fields which is not Option<T>
        let deserialized: CmdOptions =
            serde_json::from_str(options_without_option_type_fields()).unwrap();
        assert!(!deserialized.clear_envs);
        assert_eq!(
            BufferCapacity::default(),
            deserialized.output_buffer_capacity
        );

        // with some fields
        let serialized = r#"{"current_dir": "/bin", "clear_envs": true}"#;
        let deserialized: CmdOptions = serde_json::from_str(serialized).unwrap();
        let expected = CmdOptions {
            current_dir: PathBuf::from("/bin").into(),
            clear_envs: true,
            ..Default::default()
        };
        assert_eq!(expected, deserialized);

        // with none field
        let deserialized: CmdOptions = serde_json::from_str("{}").unwrap();
        assert_eq!(CmdOptions::default(), deserialized);
    }

    #[test]
    fn should_return_err_when_deserializing_invalid_options() {
        // Invalid buffer capacity = 0
        let serialized = options_json_with_invalid_buffer_capacity();
        let result = serde_json::from_str::<'_, CmdOptions>(serialized);
        let expected_msg =
            "Buffer capacity must be greater than 0 and less or equal usize::MAX / 2";
        result_contains_err_msg(&result, expected_msg);

        // Message output and logging type conflict
        let serialized = options_json_with_invalid_message_output();
        let result = serde_json::from_str::<'_, CmdOptions>(serialized);
        let expected_msg = format!(
            "Cannot use {:?} together with {:?} for stdout configuration",
            MessagingType::StandardIo,
            LoggingType::StdoutOnly
        );
        result_contains_err_msg(&result, &expected_msg);
    }

    #[test]
    fn should_properly_set_envs_during_deserialization() {
        let serialized = options_json_with_wrong_defined_envs();

        let options = serde_json::from_str::<'_, CmdOptions>(serialized).unwrap();

        let envs = options.envs.unwrap();
        assert_eq!(1, envs.len()); // PATH should be removed, because it appears in envs_to_remove
        let env = envs.get("ENV1");
        assert!(env.is_some());
        assert_eq!("value1", env.unwrap());
    }

    fn create_options() -> CmdOptions {
        let mut envs = HashMap::new();
        envs.insert("ENV1", "value1");
        envs.insert("PATH", "/bin");

        let mut options = CmdOptions::default();
        options.set_current_dir("/some/path".into());
        options.clear_inherited_envs(true);
        options.set_envs(envs);
        options.remove_env("PATH");
        options.set_message_output_buffer_capacity(BufferCapacity::try_from(24).unwrap());
        options.set_message_input(MessagingType::StandardIo);
        options
            .set_message_output(MessagingType::NamedPipe)
            .unwrap();
        options.set_logging_type(LoggingType::StderrOnly).unwrap();
        options
    }

    fn options_json() -> &'static str {
        r#"{"current_dir":"/some/path","clear_envs":true,"envs":{"ENV1":"value1"},"envs_to_remove":["PATH"],"output_buffer_capacity":24,"message_input":"StandardIo","message_output":"NamedPipe","logging_type":"StderrOnly"}"#
    }

    /// No clear_envs and output_buffer_capacity fields are set
    fn options_without_option_type_fields() -> &'static str {
        r#"{"current_dir":"/some/path","envs":{"ENV1":"value1"},"envs_to_remove":["PATH"],"message_input":"StandardIo","message_output":"NamedPipe","logging_type":"StderrOnly"}"#
    }

    fn options_json_with_invalid_buffer_capacity() -> &'static str {
        r#"{"current_dir":"/some/path","clear_envs":true,"envs":{"ENV1":"value1"},"envs_to_remove":["PATH"],"output_buffer_capacity":0,"message_input":"StandardIo","message_output":"NamedPipe","logging_type":"StderrOnly"}"#
    }

    fn options_json_with_invalid_message_output() -> &'static str {
        r#"{"current_dir":"/some/path","clear_envs":true,"envs":{"ENV1":"value1"},"envs_to_remove":["PATH"],"output_buffer_capacity":7,"message_input":"StandardIo","message_output":"StandardIo","logging_type":"StdoutOnly"}"#
    }

    fn options_json_with_wrong_defined_envs() -> &'static str {
        r#"{"current_dir":"/some/path","clear_envs":true,"envs":{"ENV1":"value1","PATH":"/bin"},"envs_to_remove":["PATH"],"output_buffer_capacity":24,"message_input":"StandardIo","message_output":"NamedPipe","logging_type":"StderrOnly"}"#
    }

    fn result_contains_err_msg<T, E: ToString>(result: &Result<T, E>, expected_msg: &str) {
        assert!(result.is_err());
        assert!(result
            .as_ref()
            .err()
            .unwrap()
            .to_string()
            .contains(expected_msg));
    }

    // Cmd -------------------------------------------------

    #[test]
    fn should_serialize_and_deserialize_cmd() {
        let options = create_options();
        let cmd = Cmd::with_args_and_options("ls", ["-l", "/bin"], options);

        let serialized = serde_json::to_string(&cmd).unwrap();
        let deserialized = serde_json::from_str(&serialized).unwrap();

        assert_eq!(cmd, deserialized);
    }

    #[test]
    fn should_serialize_and_deserialize_cmd_with_defaults() {
        // with no options
        let expected = Cmd::with_args("ls", ["-l", "/bin"]);
        let serialized = r#"{"cmd":"ls","args":["-l","/bin"]}"#;
        let deserialized = serde_json::from_str(serialized).unwrap();
        assert_eq!(expected, deserialized);

        // with no options and args
        let expected = Cmd::new("ls");
        let serialized = r#"{"cmd":"ls"}"#;
        let deserialized = serde_json::from_str(serialized).unwrap();
        assert_eq!(expected, deserialized);
    }

    // Script ----------------------------------------------

    #[test]
    fn should_serialize_and_deserialize_script() {
        let options = create_options();
        let run_config = ScriptRunConfig::new("bash", ["-C"], "sh");
        let lang = ScriptingLanguage::Other(run_config);
        let content = r#"
            echo Hello
            echo World
        "#;
        let script = Script::with_args_and_options(lang, content, ["-l", "/bin"], options);

        let serialized = serde_json::to_string(&script).unwrap();
        let deserialized = serde_json::from_str(&serialized).unwrap();

        assert_eq!(script, deserialized);
    }

    #[test]
    fn should_deserialize_script_with_defaults() {
        let lang = ScriptingLanguage::Bash;
        let content = "echo";

        // no options and no language
        let script = Script::with_args(lang.clone(), content, ["Hello", "World"]);
        let serialized = r#"{"content":"echo","args":["Hello","World"]}"#;
        let deserialized = serde_json::from_str(serialized).unwrap();
        assert_eq!(script, deserialized);

        // no options, language and args
        let script = Script::new(lang, content);
        let serialized = r#"{"content":"echo"}"#;
        let deserialized = serde_json::from_str(serialized).unwrap();
        assert_eq!(script, deserialized);
    }
}
