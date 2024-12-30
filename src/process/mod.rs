mod log_reader;
mod manager;
mod model;
mod process_handle;
mod reader;
mod serde;
mod spawner;
mod writer;

pub use manager::{
    GetLogsError, GetProcessDataError, KillProcessError, ProcessManager, ProcessManagerHandle,
    ReadMessageError, ReceiveMessageBytesError, ReceiveMessageError, SpawnProcessError,
    WriteMessageError,
};

pub use log_reader::LogsQuery;

pub use model::{
    Cmd, CmdOptions, CmdOptionsError, CustomScriptRunConfig, LoggingType, MessagingType,
    ProcessData, ProcessId, Runnable, Script, ScriptLanguage, SCRIPT_FILE_PATH_PLACEHOLDER,
};

pub use process_handle::ProcessHandle;

#[cfg(any(feature = "json", feature = "message-pack"))]
pub use serde::DataFormat;

#[cfg(feature = "message-pack")]
pub use serde::Encoding;

#[cfg(feature = "builder")]
pub use model::{
    CmdBuilder, CmdBuilderError, CmdOptionsBuilder, CmdOptionsBuilderError, ScriptBuilder,
    ScriptBuilderError,
};
