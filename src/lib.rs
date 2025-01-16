//! # Proc-heim
//!
//! `Proc-heim` is a library for running and managing short-lived and long-lived processes using asynchronous API. A new process can be created by running either command or script.
//!
//! ## Features
//! `Proc-heim` internally uses [`tokio::process`](https://docs.rs/tokio/latest/tokio/process/index.html) for executing processes and provides all its functionality plus additional features:
//!  * spawning new processes via scripts (in different scripting languages) and any `Rust` types, which implements `Runnable` trait,
//!  * flexible managing of all spawned processes using single facade, which can be easily shared by multiple threads/tasks,
//!  * bi-directional, message-based communication between child and parent processes via standard IO streams or named pipes,
//!  * collecting and querying logs produced by child processes (both running and completed).
//!
//! For more detailed list of features see [`ProcessManagerHandle`](struct@crate::manager::ProcessManagerHandle) documentation.
//!
//! ## API overview
//! `Proc-heim` library is divided into two modules: `model` and `manager`.
//!
//! The first one defines types and traits used to describe commands, scripts and their settings, such as  messaging and logging type, environment variables and working directory. The module contains a `Runnable` trait, which defines how to run a user-defined process. The library provides two implementation of this trait: `Cmd` and `Script`.
//!
//! The `manager` module provides an API for spawning and managing child processes. The whole implementation relies on `Actor model` architecture. To start using the library, a client code needs to spawn a `ProcessManager` task, responsible for:
//! * creating new actors implementing some functionality (eg. reading messages from child process),
//! * forwarding messages sent between client code and other actors.
//!
//! After spawning the `ProcessManager` task, a `ProcessManagerHandle` is being returned, which exposes an API for spawning and managing user-defined processes.
//!
//! ## Examples
//!
//! ### Spawning a new `ProcessManager` task
//! ```no_run
//! use proc_heim::manager::ProcessManager;
//! use std::path::PathBuf;
//!
//! let working_directory = PathBuf::from("/some/temp/path");
//! let handle = ProcessManager::spawn(working_directory).expect("Invalid working directory");
//! // now use the handle to spawn new processes and interact with them
//! ```
//!
//! ### Spawning a new process from command
//! ```no_run
//! # #[tokio::main] async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use proc_heim::manager::ProcessManager;
//! use std::path::PathBuf;
//! use proc_heim::model::command::Cmd;
//!
//! let working_directory = PathBuf::from("/tmp/proc_heim");
//! let handle = ProcessManager::spawn(working_directory)?;
//! let cmd = Cmd::with_args("ls", ["-l", "/some/dir"]);
//! let process_id = handle.spawn(cmd).await?;
//! # Ok(()) }
//! // now use the process_id to interact with a process ...
//! ```
//!
//! ### Reading logs from a process
//! ```no_run
//! # #[tokio::main] async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use proc_heim::{
//!     manager::{LogsQuery, ProcessManager},
//!     model::{
//!         command::{CmdOptions, LoggingType},
//!         script::{Script, ScriptingLanguage}
//!     },
//! };
//! use std::{path::PathBuf, time::Duration};
//!
//! let working_directory = PathBuf::from("/tmp/proc_heim");
//! let handle = ProcessManager::spawn(working_directory)?;
//! let script = Script::with_args_and_options(
//!     ScriptingLanguage::Bash,
//!     r#"
//!     echo 'Simple log example'
//!     echo "Hello $1"
//!     echo 'Error log' >&2
//!     "#,
//!     ["World"],
//!     CmdOptions::with_logging(LoggingType::StdoutAndStderr),
//! );
//!
//! let process_id = handle.spawn(script).await?;
//! // We are waiting for the process to exit in order to get all logs
//! handle.wait(process_id, Duration::from_micros(10)).await??;
//!
//! let logs = handle
//!     .get_logs_stdout(process_id, LogsQuery::with_offset(1))
//!     .await?;
//! assert_eq!(1, logs.len());
//! assert_eq!("Hello World", logs[0]);
//!
//! let error_logs = handle
//!     .get_logs_stderr(process_id, LogsQuery::fetch_all())
//!     .await?;
//! assert_eq!(1, error_logs.len());
//! assert_eq!("Error log", error_logs[0]);
//! # Ok(()) }
//! ```
//!
//! ### Messaging with a process via named pipes
//!
//! ```no_run
//! use futures::TryStreamExt;
//! use proc_heim::{
//!     manager::ProcessManager,
//!     model::{
//!         command::CmdOptions,
//!         script::{Script, ScriptingLanguage},
//!     },
//! };
//! use std::path::PathBuf;
//!
//! # #[tokio::main] async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let working_directory = PathBuf::from("/tmp/proc_heim");
//! let handle = ProcessManager::spawn(working_directory)?;
//! let script = Script::with_options(
//!     ScriptingLanguage::Bash,
//!     r#"
//!     counter=0
//!     while read msg; do
//!         echo "$counter: $msg" > $OUTPUT_PIPE
//!         counter=$((counter + 1))
//!     done < $INPUT_PIPE
//!     "#,
//!     CmdOptions::with_named_pipe_messaging(), // we want to send messages bidirectionally
//! );
//!
//! // We can use "spawn_with_handle" instead of "spawn" to get "ProcessHandle",
//! // which mimics the "ProcessManagerHandle" API,
//! // but without having to pass the process ID to each method call.
//! let process_handle = handle.spawn_with_handle(script).await?;
//!
//! process_handle.send_message("First message").await?;
//! // We can send a next message without causing a deadlock here.
//! // This is possible because the response to the first message
//! // will be read by a dedicated Tokio task,
//! // spawned automatically by the Process Manager.
//! process_handle.send_message("Second message").await?;
//!
//! let mut stream = process_handle.subscribe_message_string_stream().await?;
//!
//! let msg = stream.try_next().await?.unwrap();
//! assert_eq!("0: First message", msg);
//!
//! let msg = stream.try_next().await?.unwrap();
//! assert_eq!("1: Second message", msg);
//!
//! let result = process_handle.kill().await;
//! assert!(result.is_ok());
//! # Ok(()) }
//! ```
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
            BufferCapacity, Cmd, CmdError, CmdOptions, CmdOptionsError, LoggingType, MessagingType,
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
        //! The most important types of this module are [`Script`] and [`ScriptingLanguage`] which enables of defining a scripts.
        //! `Script` internally store script's content in a file and then executes [`Cmd`](struct@crate::model::command::Cmd).
        //! [`ScriptingLanguage`] defines the language in which the script is implemented.
        //! Currently, library supports 8 most popular scripting languages, but it is possible to support a custom ones via [`ScriptingLanguage::Other`].
        //!
        //! [`Script`] implements [`Runnable`](trait@crate::model::Runnable) trait, and therefore it can be spawned using [`ProcessManagerHandle`](struct@crate::manager::ProcessManagerHandle).
        pub use crate::process::{
            Script, ScriptRunConfig, ScriptingLanguage, SCRIPT_FILE_PATH_PLACEHOLDER,
        };

        /// Alternative API for creating [`Script`] structure.
        #[cfg(feature = "builder")]
        pub mod builder {
            pub use crate::process::{ScriptBuilder, ScriptBuilderError};
        }
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

    #[cfg(any(feature = "json", feature = "message-pack"))]
    pub mod serde {
        //! Types representing messages format and encoding.
        pub use crate::process::MessageFormat;

        #[cfg(feature = "message-pack")]
        pub use crate::process::Encoding;
    }
}
// TODO: add info about features. (and in docs in proper places eg. for serde module)
