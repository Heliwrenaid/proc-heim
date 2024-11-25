mod manager;
mod model;
mod reader;
mod spawner;
mod writer;

pub use manager::{
    KillProcessError, ReadMessageError, ReceiveMessageError, SpawnProcessError, WriteMessageError,
};
pub use manager::{ProcessManager, ProcessManagerHandle};
pub use model::ProcessId;
pub use model::{
    Cmd, CmdBuilder, CmdBuilderError, CmdOptions, CmdOptionsBuilder, CmdOptionsBuilderError,
};
