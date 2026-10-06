use super::*;
use crate::ControlInputs;
use crate::bridge::wire::{Request, RequestRef, RequestType};

fn split(frame: &[u8]) -> (usize, &[u8]) {
    let (header, payload) = frame.split_at(FRAME_HEADER_LEN);
    (frame_len(header.try_into().unwrap()).unwrap(), payload)
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

fn header(len: usize) -> [u8; FRAME_HEADER_LEN] {
    (len as u32).to_be_bytes()
}

#[test]
fn frame_len_accepts_max() {
    assert_eq!(frame_len(header(MAX_FRAME_LEN)).unwrap(), MAX_FRAME_LEN);
}

#[test]
fn frame_len_rejects_oversize() {
    let err = frame_len(header(MAX_FRAME_LEN + 1)).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::InvalidData);

    let err = frame_len([0xFF; FRAME_HEADER_LEN]).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::InvalidData);
}

#[test]
fn encode_frame_into_matches_encode_frame() {
    let request = Request {
        request_type: RequestType::ExchangeData,
        payload: Some(ControlInputs::default()),
    };
    let mut buf = b"stale contents".to_vec();

    encode_frame_into(&request, &mut buf).unwrap();

    assert_eq!(buf, encode_frame(&request).unwrap());
}

#[test]
fn encode_frame_into_reuses_buffer() {
    let mut buf = Vec::with_capacity(1024);
    let ptr = buf.as_ptr();

    encode_frame_into(&RequestType::EnableRC, &mut buf).unwrap();
    encode_frame_into(&RequestType::ResetAircraft, &mut buf).unwrap();

    assert_eq!(buf.as_ptr(), ptr);
    assert_eq!(buf, encode_frame(&RequestType::ResetAircraft).unwrap());
}

#[test]
fn encode_rejects_oversize_payload() {
    // Byte-sequence length varint plus content pushes the payload over the limit
    let payload = vec![0u8; MAX_FRAME_LEN];

    match encode_frame(&payload) {
        Err(BridgeError::Connection(e)) => assert_eq!(e.kind(), io::ErrorKind::InvalidData),
        other => panic!("expected Connection(InvalidData), got {:?}", other),
    }
}
