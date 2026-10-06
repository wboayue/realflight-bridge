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

/// Largest accepted payload. Real messages are a few hundred bytes; the limit
/// stops a peer from forcing huge allocations via the length prefix.
pub(crate) const MAX_FRAME_LEN: usize = 64 * 1024;

/// Encodes a message as a complete frame (length prefix + payload).
#[cfg(any(test, feature = "bench-internals"))]
pub fn encode_frame<T: Serialize>(message: &T) -> Result<Vec<u8>, BridgeError> {
    let mut frame = Vec::new();
    encode_frame_into(message, &mut frame)?;
    Ok(frame)
}

/// Encodes a message as a complete frame into `buf`, replacing its contents.
pub fn encode_frame_into<T: Serialize>(message: &T, buf: &mut Vec<u8>) -> Result<(), BridgeError> {
    let mut frame = std::mem::take(buf);
    frame.clear();
    frame.extend_from_slice(&[0u8; FRAME_HEADER_LEN]);
    let mut frame = postcard::to_extend(message, frame).map_err(codec_error)?;
    let len = frame.len() - FRAME_HEADER_LEN;
    if len > MAX_FRAME_LEN {
        return Err(BridgeError::Connection(oversize_error(len)));
    }
    frame[..FRAME_HEADER_LEN].copy_from_slice(&(len as u32).to_be_bytes());
    *buf = frame;
    Ok(())
}

/// Returns the payload length announced by a frame header.
///
/// Fails with [io::ErrorKind::InvalidData] if it exceeds [MAX_FRAME_LEN].
pub(crate) fn frame_len(header: [u8; FRAME_HEADER_LEN]) -> io::Result<usize> {
    let len = u32::from_be_bytes(header) as usize;
    if len > MAX_FRAME_LEN {
        return Err(oversize_error(len));
    }
    Ok(len)
}

/// Decodes a frame payload (without the length prefix).
pub fn decode_frame<T: DeserializeOwned>(payload: &[u8]) -> Result<T, BridgeError> {
    postcard::from_bytes(payload).map_err(codec_error)
}

fn oversize_error(len: usize) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("frame of {len} bytes exceeds limit of {MAX_FRAME_LEN}"),
    )
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
