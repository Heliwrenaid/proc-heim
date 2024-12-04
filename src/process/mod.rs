mod manager;
mod model;
mod reader;
mod serde;
mod spawner;
mod writer;

pub use manager::{
    KillProcessError, ProcessManager, ProcessManagerHandle, ReadMessageError,
    ReceiveMessageBytesError, ReceiveMessageError, SpawnProcessError, WriteMessageError,
};

#[cfg(any(feature = "json", feature = "message-pack"))]
pub use serde::{DataFormat, Encoding};

pub use model::ProcessId;
pub use model::{
    Cmd, CmdBuilder, CmdBuilderError, CmdOptions, CmdOptionsBuilder, CmdOptionsBuilderError,
    MessagingType,
};
