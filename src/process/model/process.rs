use derive_builder::Builder;
use tokio::process::Child;

use crate::process::{reader::MessageReaderHandle, writer::MessageWriterHandle};

#[derive(Builder, Debug)]
#[builder(pattern = "owned")]
pub struct Process {
    pub child: Child,
    #[builder(default = "None")]
    pub message_writer: Option<MessageWriterHandle>,
    #[builder(default = "None")]
    pub message_reader: Option<MessageReaderHandle>,
}
