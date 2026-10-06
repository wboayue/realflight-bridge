//! Length-prefixed postcard framing shared by remote bridges and the proxy.
//! Runtime-agnostic (no I/O).
//!
//! Wire format: 4-byte big-endian payload length, then postcard-encoded payload.

use std::io;

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::BridgeError;

/// Size of the length prefix.
pub(crate) const FRAME_HEADER_LEN: usize = 4;

/// Encodes a message as a complete frame (length prefix + payload).
pub(crate) fn encode_frame<T: Serialize>(message: &T) -> Result<Vec<u8>, BridgeError> {
    let mut frame =
        postcard::to_extend(message, vec![0u8; FRAME_HEADER_LEN]).map_err(codec_error)?;
    let len = u32::try_from(frame.len() - FRAME_HEADER_LEN).map_err(codec_error)?;
    frame[..FRAME_HEADER_LEN].copy_from_slice(&len.to_be_bytes());
    Ok(frame)
}

/// Returns the payload length announced by a frame header.
pub(crate) fn frame_len(header: [u8; FRAME_HEADER_LEN]) -> usize {
    u32::from_be_bytes(header) as usize
}

/// Decodes a frame payload (without the length prefix).
pub(crate) fn decode_frame<T: DeserializeOwned>(payload: &[u8]) -> Result<T, BridgeError> {
    postcard::from_bytes(payload).map_err(codec_error)
}

/// Codec failures are reported as invalid data on the connection.
fn codec_error<E>(e: E) -> BridgeError
where
    E: std::error::Error + Send + Sync + 'static,
{
    BridgeError::Connection(io::Error::new(io::ErrorKind::InvalidData, e))
}

#[cfg(test)]
mod tests;
