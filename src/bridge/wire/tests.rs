//! Tests for wire protocol response interpretation.

use crate::{BridgeError, SimulatorState};

use super::frame::{FRAME_HEADER_LEN, decode_frame, encode_frame};
use super::{RemoteError, RemoteErrorKind, Response, ResponseStatus};

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
    fn round_trips_every_kind() {
        let cases = [
            (
                BridgeError::Connection(std::io::Error::other("refused")),
                RemoteErrorKind::Connection,
            ),
            (
                BridgeError::Initialization("no pool".into()),
                RemoteErrorKind::Initialization,
            ),
            (
                BridgeError::SoapFault("rejected".into()),
                RemoteErrorKind::SoapFault,
            ),
            (
                BridgeError::Parse {
                    field: "airspeed".into(),
                    message: "bad float".into(),
                },
                RemoteErrorKind::Parse,
            ),
            (
                BridgeError::Protocol("truncated".into()),
                RemoteErrorKind::Protocol,
            ),
        ];

        for (err, kind) in cases {
            assert_eq!(RemoteError::from(&err).kind, kind);

            let expected = err.to_string();
            let rebuilt = round_trip(err);
            assert_eq!(RemoteError::from(&rebuilt).kind, kind);
            assert_eq!(rebuilt.to_string(), expected);
        }
    }

    #[test]
    fn parse_keeps_field() {
        let err = BridgeError::Parse {
            field: "airspeed".into(),
            message: "bad float".into(),
        };
        match round_trip(err) {
            BridgeError::Parse { field, message } => {
                assert_eq!(field, "airspeed");
                assert_eq!(message, "bad float");
            }
            other => panic!("expected Parse, got {:?}", other),
        }
    }
}
