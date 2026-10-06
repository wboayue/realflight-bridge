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
mod tests {
    use super::*;
    use crate::ControlInputs;
    use crate::bridge::remote::{Request, RequestRef, RequestType};

    fn split(frame: &[u8]) -> (usize, &[u8]) {
        let (header, payload) = frame.split_at(FRAME_HEADER_LEN);
        (frame_len(header.try_into().unwrap()), payload)
    }

    #[test]
    fn prefixes_payload_length() {
        let frame = encode_frame(&RequestType::ResetAircraft).unwrap();
        let (len, payload) = split(&frame);
        assert_eq!(len, payload.len());
        assert_eq!(
            payload,
            postcard::to_stdvec(&RequestType::ResetAircraft).unwrap()
        );
    }

    #[test]
    fn round_trips() {
        let mut control = ControlInputs::default();
        control.channels[3] = 0.25;
        let request = Request {
            request_type: RequestType::ExchangeData,
            payload: Some(control.clone()),
        };

        let frame = encode_frame(&request).unwrap();
        let decoded: Request = decode_frame(split(&frame).1).unwrap();

        assert_eq!(decoded.request_type, RequestType::ExchangeData);
        assert_eq!(decoded.payload, Some(control));
    }

    #[test]
    fn request_ref_is_wire_compatible_with_request() {
        let control = ControlInputs::default();
        let borrowed = RequestRef {
            request_type: RequestType::ExchangeData,
            payload: Some(&control),
        };
        let owned = Request {
            request_type: RequestType::ExchangeData,
            payload: Some(control.clone()),
        };

        assert_eq!(
            encode_frame(&borrowed).unwrap(),
            encode_frame(&owned).unwrap()
        );
    }

    #[test]
    fn garbage_is_invalid_data() {
        match decode_frame::<Request>(&[0xFF, 0xFF, 0xFF, 0xFF]) {
            Err(BridgeError::Connection(e)) => assert_eq!(e.kind(), io::ErrorKind::InvalidData),
            other => panic!("expected Connection(InvalidData), got {:?}", other),
        }
    }

    #[test]
    fn truncated_payload_is_invalid_data() {
        let frame = encode_frame(&Request {
            request_type: RequestType::ExchangeData,
            payload: Some(ControlInputs::default()),
        })
        .unwrap();
        let payload = &split(&frame).1[..8];

        assert!(matches!(
            decode_frame::<Request>(payload),
            Err(BridgeError::Connection(_))
        ));
    }
}
