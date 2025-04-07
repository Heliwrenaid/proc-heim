mod log_reader;
mod manager;
mod message;
mod message_stream;
mod model;
mod process_handle;
mod reader;
mod scoped_process_handle;
mod serde;
mod spawner;
mod writer;

pub use log_reader::LogsQuery;
pub use manager::*;
pub use message::Message;
pub use message_stream::{MessageStreamExt, ResultStreamExt, TryMessageStreamExt};
pub use model::*;
pub use process_handle::ProcessHandle;
pub use scoped_process_handle::ScopedProcessHandle;
#[cfg(any(feature = "json", feature = "message-pack"))]
pub use serde::SerdeError;
pub use spawner::{INPUT_PIPE_ENV_NAME, OUTPUT_PIPE_ENV_NAME, PROCESS_DATA_DIR_ENV_NAME};

#[cfg(any(feature = "json", feature = "message-pack"))]
pub use serde::MessageFormat;

#[cfg(feature = "message-pack")]
pub use serde::Encoding;
