//! Tests for wire protocol response interpretation.

use crate::{BridgeError, SimulatorState};

use super::{Response, ResponseStatus};

mod response_helpers {
    use super::*;

    fn response(status: ResponseStatus, payload: Option<SimulatorState>) -> Response {
        Response { status, payload }
    }

    #[test]
    fn into_state_returns_payload() {
        let state = SimulatorState::default();
        let result = response(ResponseStatus::Success, Some(state.clone())).into_state();
        assert_eq!(result.unwrap(), state);
    }

    #[test]
    fn into_state_without_payload_is_error() {
        match response(ResponseStatus::Success, None).into_state() {
            Err(BridgeError::SoapFault(msg)) => assert!(msg.contains("No payload")),
            other => panic!("expected SoapFault, got {:?}", other),
        }
    }

    #[test]
    fn into_unit_ok_on_success() {
        assert!(response(ResponseStatus::Success, None).into_unit().is_ok());
    }

    #[test]
    fn into_unit_fault_on_error_status() {
        match response(ResponseStatus::Error, None).into_unit() {
            Err(BridgeError::SoapFault(msg)) => assert!(msg.contains("Proxy reported")),
            other => panic!("expected SoapFault, got {:?}", other),
        }
    }

    #[test]
    fn into_state_fault_on_error_status() {
        match response(ResponseStatus::Error, None).into_state() {
            Err(BridgeError::SoapFault(msg)) => assert!(msg.contains("Proxy reported")),
            other => panic!("expected SoapFault, got {:?}", other),
        }
    }
}
