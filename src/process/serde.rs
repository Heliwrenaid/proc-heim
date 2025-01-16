#[cfg(any(feature = "json", feature = "message-pack"))]
pub use inner::{MessageFormat, SerdeUtil};

#[cfg(feature = "message-pack")]
pub use inner::Encoding;

#[cfg(any(feature = "json", feature = "message-pack"))]
mod inner {
    #[cfg(feature = "message-pack")]
    use base64::prelude::*;

    use std::fmt::Display;

    /// Message data format type.
    #[derive(Debug, Clone)]
    #[non_exhaustive]
    pub enum MessageFormat {
        /// `JSON` format.
        #[cfg(feature = "json")]
        Json,
        /// `MessagePack` binary format.
        /// `MessagePack` serialization can produce `null bytes`, which will break a communication via pipes.
        /// Therefore the messages should be additionally encoded.
        #[cfg(feature = "message-pack")]
        MessagePack(Encoding),
    }

    /// Enum describing message encoding.
    #[cfg(feature = "message-pack")]
    #[derive(Debug, Clone)]
    pub enum Encoding {
        /// `Base64` encoding.
        Base64,
        /// `Hexadecimal` encoding.
        Hex,
    }

    impl Display for MessageFormat {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                #[cfg(feature = "json")]
                MessageFormat::Json => f.write_str("Json"),
                #[cfg(feature = "message-pack")]
                MessageFormat::MessagePack(_encoding) => f.write_str("MessagePack"),
            }
        }
    }

    #[cfg(feature = "message-pack")]
    impl Display for Encoding {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Encoding::Base64 => f.write_str("Base64"),
                Encoding::Hex => f.write_str("Hex"),
            }
        }
    }

    #[allow(clippy::enum_variant_names)]
    #[derive(thiserror::Error, Debug)]
    pub enum SerdeError {
        #[error("Cannot serialize data with format: {0}. Cause: {1}")]
        SerializationError(MessageFormat, String),
        #[error("Cannot deserialize data with format: {0}. Cause: {1}")]
        DeserializationError(MessageFormat, String),
        #[error("Cannot decode data with {0}. Cause: {1}")]
        #[cfg(feature = "message-pack")]
        DecodingError(Encoding, String),
    }

    pub struct SerdeUtil {}

    impl SerdeUtil {
        pub fn serialize<T: serde::Serialize>(
            data: &T,
            format: &MessageFormat,
        ) -> Result<Vec<u8>, SerdeError> {
            let bytes = match format {
                #[cfg(feature = "json")]
                MessageFormat::Json => serde_json::to_vec(&data)
                    .map_err(|err| SerdeError::SerializationError(format.clone(), err.to_string())),
                #[cfg(feature = "message-pack")]
                MessageFormat::MessagePack(ref encoding) => {
                    let bytes = rmp_serde::to_vec(&data).map_err(|err| {
                        SerdeError::SerializationError(format.clone(), err.to_string())
                    })?;
                    Ok(match encoding {
                        Encoding::Base64 => BASE64_STANDARD.encode(bytes),
                        Encoding::Hex => hex::encode(bytes),
                    }
                    .into_bytes())
                }
            }?;
            Ok(bytes)
        }

        pub fn deserialize<T: serde::de::DeserializeOwned>(
            bytes: &[u8],
            format: &MessageFormat,
        ) -> Result<T, SerdeError> {
            match format {
                #[cfg(feature = "json")]
                MessageFormat::Json => serde_json::from_slice(bytes).map_err(|err| {
                    SerdeError::DeserializationError(format.clone(), err.to_string())
                }),
                #[cfg(feature = "message-pack")]
                MessageFormat::MessagePack(ref encoding) => {
                    let decoded_bytes = match encoding {
                        Encoding::Base64 => {
                            BASE64_STANDARD.decode(bytes).map_err(|err| err.to_string())
                        }
                        Encoding::Hex => hex::decode(bytes).map_err(|err| err.to_string()),
                    }
                    .map_err(|err_source_msg| {
                        SerdeError::DecodingError(encoding.clone(), err_source_msg)
                    })?;
                    rmp_serde::from_slice(&decoded_bytes).map_err(|err| {
                        SerdeError::DeserializationError(format.clone(), err.to_string())
                    })
                }
            }
        }
    }
}
