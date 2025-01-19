use super::{Message, ReceiveDeserializedMessageError, ReceiveMessageError};
use std::{fmt::Debug, string::FromUtf8Error};

#[cfg(any(feature = "json", feature = "message-pack"))]
use super::{MessageFormat, SerdeError};

use futures::{future, Stream, StreamExt};

impl<S: ?Sized> MessageStreamExt for S where S: Stream<Item = Message> {}

/// Adapters specific to `Message`-returning streams.
pub trait MessageStreamExt: Stream<Item = Message> {
    /// Convert stream's messages into bytes.
    fn into_bytes_stream(self) -> impl Stream<Item = Vec<u8>>
    where
        Self: Sized,
    {
        self.map(Message::into)
    }

    /// Convert stream's messages into strings.
    /// If the message cannot be deserialized from bytes, the [`FromUtf8Error`] error will be returned.
    fn into_string_stream(self) -> impl Stream<Item = Result<String, FromUtf8Error>>
    where
        Self: Sized,
    {
        self.map(Message::try_into_string)
    }

    /// Convert stream's messages into `Rust` types implementing `Deserialize` trait.
    /// If the message cannot be deserialized from bytes, the [`SerdeError::DeserializationFailure`] error will be returned.
    #[cfg(any(feature = "json", feature = "message-pack"))]
    fn into_deserialized_stream<T>(
        self,
        format: &MessageFormat,
    ) -> impl Stream<Item = Result<T, SerdeError>>
    where
        T: for<'de> serde::Deserialize<'de>,
        Self: Sized,
    {
        self.map(|message| message.deserialize(format))
    }
}

impl<S: ?Sized> TryMessageStreamExt for S where
    S: Stream<Item = Result<Message, ReceiveMessageError>>
{
}

/// Adapters for streams yielding `Result<Message, ReceiveMessageError>`.
pub trait TryMessageStreamExt: Stream<Item = Result<Message, ReceiveMessageError>> {
    /// Convert stream's messages into bytes.
    fn into_bytes_stream(self) -> impl Stream<Item = Result<Vec<u8>, ReceiveMessageError>>
    where
        Self: Sized,
    {
        self.map(|result| result.map(Message::into_bytes))
    }

    /// Convert stream's messages into strings.
    /// If the message cannot be deserialized from bytes, the [`ReceiveDeserializedMessageError::CannotDeserializeMessage`] error will be returned.
    fn into_string_stream(
        self,
    ) -> impl Stream<Item = Result<String, ReceiveDeserializedMessageError>>
    where
        Self: Sized,
    {
        self.map(to_string)
    }

    /// Convert stream's messages into `Rust` types implementing `Deserialize` trait.
    /// If the message cannot be deserialized from bytes, the [`ReceiveDeserializedMessageError::CannotDeserializeMessage`] error will be returned.
    #[cfg(any(feature = "json", feature = "message-pack"))]
    fn into_deserialized_stream<T>(
        self,
        format: &MessageFormat,
    ) -> impl Stream<Item = Result<T, ReceiveDeserializedMessageError>>
    where
        T: for<'de> serde::Deserialize<'de>,
        Self: Sized,
    {
        self.map(|result| deserialize_message(result, format))
    }

    /// Ignore lost messages (see [`ProcessManagerHandle::subscribe_message_stream`](crate::manager::ProcessManagerHandle::subscribe_message_stream)), returning a stream yielding `Message` types.
    /// The returned stream can be transformed further using [`MessageStreamExt`] trait.
    fn ignore_lost_messages(self) -> impl Stream<Item = Message>
    where
        Self: Sized,
    {
        self.filter(|result| future::ready(result.is_ok()))
            .map(Result::unwrap)
    }
}

#[allow(unused)]
fn to_string(
    result: Result<Message, ReceiveMessageError>,
) -> Result<String, ReceiveDeserializedMessageError> {
    result?.try_into_string().map_err(|err| {
        ReceiveDeserializedMessageError::CannotDeserializeMessage(format!(
            "Cannot deserialize data from raw bytes to string: {err}"
        ))
    })
}

#[cfg(any(feature = "json", feature = "message-pack"))]
fn deserialize_message<T: for<'de> serde::Deserialize<'de>>(
    result: Result<Message, ReceiveMessageError>,
    format: &MessageFormat,
) -> Result<T, ReceiveDeserializedMessageError> {
    result?
        .deserialize(format)
        .map_err(|err| ReceiveDeserializedMessageError::CannotDeserializeMessage(err.to_string()))
}

impl<S: ?Sized, T, E: Debug> ResultStreamExt<T, E> for S where S: Stream<Item = Result<T, E>> {}

/// Adapters specific to `Result`-returning streams.
pub trait ResultStreamExt<T, E: Debug>: Stream<Item = Result<T, E>> {
    /// Returns a stream with filtered out `Err(T)` items.
    fn filter_ok(self) -> impl Stream<Item = T>
    where
        Self: Sized,
    {
        self.filter(|result| future::ready(result.is_ok()))
            .map(Result::unwrap)
    }
}
