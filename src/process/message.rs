#[cfg(any(feature = "json", feature = "message-pack"))]
use super::{
    serde::{SerdeError, SerdeUtil},
    MessageFormat,
};
use std::string::FromUtf8Error;

/// Type used for communication between a parent and child process.
///
/// It act as a wrapper around bytes, which enables of easy conversion from/to strings
/// and `Rust` types implementing `Serialize` and `Deserialize` traits.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Message {
    pub(crate) bytes: Vec<u8>,
}

impl From<Vec<u8>> for Message {
    fn from(value: Vec<u8>) -> Self {
        Self { bytes: value }
    }
}

impl From<&[u8]> for Message {
    fn from(value: &[u8]) -> Self {
        Self {
            bytes: value.to_vec(),
        }
    }
}

impl From<String> for Message {
    fn from(value: String) -> Self {
        Self {
            bytes: value.into(),
        }
    }
}

impl From<&str> for Message {
    fn from(value: &str) -> Self {
        Self {
            bytes: value.into(),
        }
    }
}

impl<const N: usize> From<&[u8; N]> for Message {
    fn from(value: &[u8; N]) -> Self {
        Self {
            bytes: value.to_vec(),
        }
    }
}

impl TryFrom<Message> for String {
    type Error = FromUtf8Error;

    fn try_from(value: Message) -> Result<Self, Self::Error> {
        String::from_utf8(value.bytes)
    }
}

impl AsRef<Vec<u8>> for Message {
    fn as_ref(&self) -> &Vec<u8> {
        &self.bytes
    }
}

impl From<Message> for Vec<u8> {
    fn from(value: Message) -> Self {
        value.bytes
    }
}

impl Message {
    /// Create new message from given bytes.
    pub fn from_bytes<T>(bytes: T) -> Self
    where
        T: Into<Vec<u8>>,
    {
        Self {
            bytes: bytes.into(),
        }
    }

    /// Create new message from given string.
    pub fn from_string<S>(value: S) -> Self
    where
        S: Into<String>,
    {
        Self {
            bytes: value.into().into(),
        }
    }

    /// Convert message into bytes.
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    /// Try to convert message into string.
    pub fn try_into_string(self) -> Result<String, FromUtf8Error> {
        String::from_utf8(self.bytes)
    }

    /// Deserialize message into `Rust` type implementing `Deserialize` trait.
    #[cfg(any(feature = "json", feature = "message-pack"))]
    pub fn deserialize<T>(&self, format: &MessageFormat) -> Result<T, SerdeError>
    where
        T: for<'de> serde::Deserialize<'de>,
    {
        SerdeUtil::deserialize(&self.bytes, format)
    }

    /// Create new message message from `Rust` type implementing `Serialize` trait and with given data format.
    #[cfg(any(feature = "json", feature = "message-pack"))]
    pub fn from_serializable<S>(data: &S, format: &MessageFormat) -> Result<Message, SerdeError>
    where
        S: serde::Serialize,
    {
        let bytes = SerdeUtil::serialize(data, format)?;
        Ok(Self { bytes })
    }
}
