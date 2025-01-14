// TODO: use readme in repo and crates-io. Here use docs comments.
#![doc = include_str!("../README.md")]

mod process;
mod working_dir;

pub mod model {
    //! Types and traits used for modeling commands, scripts and custom processes.
    pub mod command {
        //! Types representing commands and its settings.
        //!
        //! The two most important struct for modeling command is [`Cmd`] and [`CmdOptions`].
        //! The first one defines command name and its arguments,
        //! and the second is used to describe additional settings like command's input/outputs, working directory and environment variables.
        //!
        //! [`Cmd`] implements [`Runnable`](trait@crate::model::Runnable) trait, and therefore it can be spawned using [`ProcessManagerHandle`](struct@crate::manager::ProcessManagerHandle).
        pub use crate::process::{
            BufferCapacity, Cmd, CmdOptions, CmdOptionsError, LoggingType, MessagingType,
        };

        /// Alternative API for creating [`Cmd`] and [`CmdOptions`] structures.
        #[cfg(feature = "builder")]
        pub mod builder {
            pub use crate::process::{
                CmdBuilder, CmdBuilderError, CmdOptionsBuilder, CmdOptionsBuilderError,
            };
        }
    }

    pub mod script {
        //! Types representing scripts.
        //!
        //! The most important types of this module are [`Script`] and [`ScriptLanguage`] which enables of defining a scripts.
        //! `Script` internally store script's content in a file and then executes [`Cmd`](struct@crate::model::command::Cmd).
        //! [`ScriptLanguage`] defines the language in which the script is implemented.
        //! Currently, library supports 8 most popular scripting languages, but it is possible to support a custom ones via [`ScriptLanguage::Other`].
        //!
        //! [`Script`] implements [`Runnable`](trait@crate::model::Runnable) trait, and therefore it can be spawned using [`ProcessManagerHandle`](struct@crate::manager::ProcessManagerHandle).
        pub use crate::process::{
            CustomScriptRunConfig, Script, ScriptLanguage, SCRIPT_FILE_PATH_PLACEHOLDER,
        };

        /// Alternative API for creating [`Script`] structure.
        #[cfg(feature = "builder")]
        pub mod builder {
            pub use crate::process::{ScriptBuilder, ScriptBuilderError};
        }
    }

    // TODO: move to manager
    #[cfg(any(feature = "json", feature = "message-pack"))]
    pub mod serde {
        //! Types representing messages format and encoding.
        pub use crate::process::DataFormat;

        #[cfg(feature = "message-pack")]
        pub use crate::process::Encoding;
    }

    pub use crate::process::Runnable;
}

pub mod manager {
    //! API used for spawning and managing multiple processes.
    //!
    //! The API is implemented by [`ProcessManager`], which spawns a `Tokio` task and exposes its functionalities through [`ProcessManagerHandle`].
    //! `ProcessManagerHandle` communicates asynchronously with `ProcessManager` via message passing,
    //! therefore it can be cheaply cloned and used by many threads concurrently without any lock-based synchronization.
    //!
    //! See [`ProcessManager`] and [`ProcessManagerHandle`] docs for more information.
    pub use crate::process::{
        GetLogsError, GetProcessInfoError, KillProcessError, LogsQuery, ProcessHandle, ProcessId,
        ProcessInfo, ProcessManager, ProcessManagerHandle, ReadMessageError,
        ReceiveMessageBytesError, ReceiveMessageError, SpawnProcessError, WriteMessageError,
        INPUT_PIPE_ENV_NAME, OUTPUT_PIPE_ENV_NAME,
    };
}
// TODO: add info about features. (and in docs in proper places eg. for serde module)
