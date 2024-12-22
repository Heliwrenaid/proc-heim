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

#[cfg(any(feature = "json", feature = "message-pack"))]
pub use serde::{DataFormat, Encoding};

pub use model::{
    Cmd, CmdBuilder, CmdBuilderError, CmdOptions, CmdOptionsBuilder, CmdOptionsBuilderError,
    CustomScriptRunConfig, LoggingType, MessagingType, ProcessData, ProcessId, Runnable, Script,
    ScriptBuilder, ScriptBuilderError, ScriptLanguage, SCRIPT_FILE_PATH_PLACEHOLDER,
};

pub use process_handle::ProcessHandle;
