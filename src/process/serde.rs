#[cfg(any(feature = "json", feature = "message-pack"))]
pub use inner::{DataFormat, SerdeUtil};

#[cfg(feature = "message-pack")]
pub use inner::Encoding;

#[cfg(any(feature = "json", feature = "message-pack"))]
mod inner {
    #[cfg(feature = "message-pack")]
    use base64::prelude::*;

    use std::fmt::Display;

    #[derive(Debug, Clone)]
    #[non_exhaustive]
    pub enum DataFormat {
        #[cfg(feature = "json")]
        Json,
        #[cfg(feature = "message-pack")]
        MessagePack(Encoding),
    }

    #[cfg(feature = "message-pack")]
    #[derive(Debug, Clone)]
    pub enum Encoding {
        Base64,
        Hex,
    }

    impl Display for DataFormat {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                #[cfg(feature = "json")]
                DataFormat::Json => f.write_str("Json"),
                #[cfg(feature = "message-pack")]
                DataFormat::MessagePack(_encoding) => f.write_str("MessagePack"),
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
        SerializationError(DataFormat, String),
        #[error("Cannot deserialize data with format: {0}. Cause: {1}")]
        DeserializationError(DataFormat, String),
        #[error("Cannot decode data with {0}. Cause: {1}")]
        #[cfg(feature = "message-pack")]
        DecodingError(Encoding, String),
    }

    pub struct SerdeUtil {}

    impl SerdeUtil {
        pub fn serialize<T: serde::Serialize>(
            data: &T,
            format: &DataFormat,
        ) -> Result<Vec<u8>, SerdeError> {
            let bytes = match format {
                #[cfg(feature = "json")]
                DataFormat::Json => serde_json::to_vec(&data)
                    .map_err(|err| SerdeError::SerializationError(format.clone(), err.to_string())),
                #[cfg(feature = "message-pack")]
                DataFormat::MessagePack(ref encoding) => {
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
            format: &DataFormat,
        ) -> Result<T, SerdeError> {
            match format {
                #[cfg(feature = "json")]
                DataFormat::Json => serde_json::from_slice(bytes).map_err(|err| {
                    SerdeError::DeserializationError(format.clone(), err.to_string())
                }),
                #[cfg(feature = "message-pack")]
                DataFormat::MessagePack(ref encoding) => {
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
