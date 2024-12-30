mod process;
mod working_dir;

pub mod model {
    pub mod command {
        pub use crate::process::{Cmd, CmdOptions, CmdOptionsError, LoggingType, MessagingType};

        #[cfg(feature = "builder")]
        pub mod builder {
            pub use crate::process::{
                CmdBuilder, CmdBuilderError, CmdOptionsBuilder, CmdOptionsBuilderError,
            };
        }
    }

    pub mod script {
        pub use crate::process::{
            CustomScriptRunConfig, Script, ScriptLanguage, SCRIPT_FILE_PATH_PLACEHOLDER,
        };

        #[cfg(feature = "builder")]
        pub mod builder {
            pub use crate::process::{ScriptBuilder, ScriptBuilderError};
        }
    }

    pub mod process {
        pub use crate::process::{LogsQuery, ProcessData, ProcessId, Runnable};
    }

    #[cfg(any(feature = "json", feature = "message-pack"))]
    pub mod serde {
        pub use crate::process::DataFormat;

        #[cfg(feature = "message-pack")]
        pub use crate::process::Encoding;
    }
}

pub mod manager {
    pub use crate::process::{
        GetLogsError, GetProcessDataError, KillProcessError, ProcessHandle, ProcessManager,
        ProcessManagerHandle, ReadMessageError, ReceiveMessageBytesError, ReceiveMessageError,
        SpawnProcessError, WriteMessageError,
    };
}
