//! Tests for wire protocol response interpretation.

use crate::{BridgeError, SimulatorState};

use super::frame::{FRAME_HEADER_LEN, decode_frame, encode_frame};
use super::{RemoteError, Response, ResponseStatus};

mod fixtures {
    use super::*;

    pub fn response(status: ResponseStatus, payload: Option<SimulatorState>) -> Response {
        Response { status, payload }
    }

    pub fn error_status(err: &BridgeError) -> ResponseStatus {
        ResponseStatus::Error(err.into())
    }
}

mod response_helpers {
    use super::fixtures::*;
    use super::*;

    #[test]
    fn into_state_returns_payload() {
        let state = SimulatorState::default();
        let result = response(ResponseStatus::Success, Some(state.clone())).into_state();
        assert_eq!(result.unwrap(), state);
    }

    #[test]
    fn into_state_without_payload_is_protocol_error() {
        match response(ResponseStatus::Success, None).into_state() {
            Err(BridgeError::Protocol(msg)) => assert!(msg.contains("No payload")),
            other => panic!("expected Protocol, got {:?}", other),
        }
    }

    #[test]
    fn into_unit_ok_on_success() {
        assert!(response(ResponseStatus::Success, None).into_unit().is_ok());
    }

    #[test]
    fn into_unit_returns_remote_error() {
        let status = error_status(&BridgeError::SoapFault("rejected".into()));
        match response(status, None).into_unit() {
            Err(BridgeError::SoapFault(msg)) => assert_eq!(msg, "rejected"),
            other => panic!("expected SoapFault, got {:?}", other),
        }
    }

    #[test]
    fn into_state_returns_remote_error() {
        let status = error_status(&BridgeError::SoapFault("rejected".into()));
        match response(status, None).into_state() {
            Err(BridgeError::SoapFault(msg)) => assert_eq!(msg, "rejected"),
            other => panic!("expected SoapFault, got {:?}", other),
        }
    }
}

mod remote_error {
    use super::fixtures::*;
    use super::*;

    /// Sends `err` through the wire format and rebuilds it on the client side.
    fn round_trip(err: BridgeError) -> BridgeError {
        let frame = encode_frame(&response(error_status(&err), None)).unwrap();
        let decoded: Response = decode_frame(&frame[FRAME_HEADER_LEN..]).unwrap();
        decoded.into_unit().unwrap_err()
    }

    #[test]
    fn round_trips_every_variant() {
        let cases = [
            (
                BridgeError::Connection(std::io::Error::other("refused")),
                "Simulator error: refused",
            ),
            (
                BridgeError::Simulator(std::io::Error::other("timed out")),
                "Simulator error: timed out",
            ),
            (
                BridgeError::Initialization("no pool".into()),
                "Initialization failed: proxy: no pool",
            ),
            (
                BridgeError::SoapFault("rejected".into()),
                "SOAP fault: rejected",
            ),
            (
                BridgeError::Parse {
                    field: "airspeed".into(),
                    message: "bad float".into(),
                },
                "Parse error for field 'airspeed': bad float",
            ),
            (
                BridgeError::Protocol("truncated".into()),
                "Protocol error: truncated",
            ),
        ];

        for (err, expected) in cases {
            let sent = RemoteError::from(&err);
            let rebuilt = round_trip(err);
            assert_eq!(rebuilt.to_string(), expected);
            assert_eq!(
                std::mem::discriminant(&RemoteError::from(&rebuilt)),
                std::mem::discriminant(&sent),
            );
        }
    }

    #[test]
    fn simulator_error_keeps_io_kind() {
        use std::io::ErrorKind;

        let kinds = [
            ErrorKind::TimedOut,
            ErrorKind::ConnectionRefused,
            ErrorKind::ConnectionReset,
            ErrorKind::ConnectionAborted,
            ErrorKind::BrokenPipe,
            ErrorKind::UnexpectedEof,
            ErrorKind::Other,
        ];
        for kind in kinds {
            let err = BridgeError::Simulator(std::io::Error::new(kind, "failed"));
            match round_trip(err) {
                BridgeError::Simulator(e) => assert_eq!(e.kind(), kind),
                other => panic!("expected Simulator({kind:?}), got {other:?}"),
            }
        }
    }

    #[test]
    fn unmapped_io_kind_becomes_other() {
        let err = BridgeError::Simulator(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "denied",
        ));
        match round_trip(err) {
            BridgeError::Simulator(e) => {
                assert_eq!(e.kind(), std::io::ErrorKind::Other);
                assert_eq!(e.to_string(), "denied");
            }
            other => panic!("expected Simulator(Other), got {other:?}"),
        }
    }
}
