use std::io::Result;

use bytes::Bytes;
use serde::{Serialize, de::DeserializeOwned};

/// Helper function to serialize a generic data-type to a [`Bytes`] object.
pub fn encode_value<V: Serialize>(value: &V) -> Result<Bytes> {
    let encoded = bincode::serde::encode_to_vec(value, bincode::config::standard()).map_err(|err| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, format!("failed to encode metadata value: {err}"))
    })?;
    Ok(Bytes::from(encoded))
}

/// Helper function to deserialize a u8 slice to a typed object.
pub fn decode_value<V: DeserializeOwned>(data: &[u8]) -> Result<V> {
    let (value, consumed): (V, usize) =
        bincode::serde::decode_from_slice(data, bincode::config::standard()).map_err(|err| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, format!("failed to decode metadata value: {err}"))
        })?;
    if consumed != data.len() {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "metadata value has trailing bytes"));
    }
    Ok(value)
}
