//! Tests for RealFlightRemoteBridge.
//!
//! Organized into submodules:
//! - `connection_tests`: Connection establishment and timeout tests
//! - `operation_tests`: Tests for bridge operations (enable_rc, disable_rc, etc.)
//! - `error_handling`: Tests for error conditions and edge cases

use std::{io::ErrorKind, time::Duration};

use crate::{BridgeError, ControlInputs, RealFlightBridge, SimulatorState};

use super::test_support::MockProxy;
use super::{RealFlightRemoteBridge, RequestType, Response, ResponseStatus};

// ============================================================================
// Connection Tests
// ============================================================================

/// Tests connecting to a non-existent server - should fail with connection refused
#[test]
fn test_connection_failure() {
    // Attempt to connect to a port where no server is running
    let result = RealFlightRemoteBridge::new("127.0.0.1:1");

    assert!(result.is_err());
    if let Err(e) = result {
        assert_eq!(e.kind(), ErrorKind::ConnectionRefused);
    }
}

/// Tests custom timeout functionality
#[test]
fn test_with_timeout_connection_failure() {
    let start = std::time::Instant::now();
    let result = RealFlightRemoteBridge::with_timeout("127.0.0.1:1", Duration::from_millis(100));
    let elapsed = start.elapsed();

    assert!(result.is_err());
    // Should fail relatively quickly (within reasonable margin of timeout)
    assert!(elapsed < Duration::from_secs(2));
}

/// Tests invalid address handling
#[test]
fn test_invalid_address() {
    let result = RealFlightRemoteBridge::new("not-a-valid-address");
    assert!(result.is_err());
}

// ============================================================================
// Operation Tests
// ============================================================================

fn success(payload: Option<SimulatorState>) -> Response {
    Response {
        status: ResponseStatus::Success,
        payload,
    }
}

#[test]
fn unit_ops_send_matching_requests() {
    let proxy = MockProxy::respond(success(None));
    let client = RealFlightRemoteBridge::new(&proxy.addr).unwrap();

    client.enable_rc().unwrap();
    client.disable_rc().unwrap();
    client.reset_aircraft().unwrap();
    drop(client);

    let types: Vec<_> = proxy
        .requests()
        .into_iter()
        .map(|r| r.request_type)
        .collect();
    assert_eq!(
        types,
        [
            RequestType::EnableRC,
            RequestType::DisableRC,
            RequestType::ResetAircraft
        ]
    );
}

#[test]
fn exchange_data_sends_inputs_and_returns_state() {
    let proxy = MockProxy::respond(success(Some(SimulatorState::default())));
    let client = RealFlightRemoteBridge::new(&proxy.addr).unwrap();
    let mut control = ControlInputs::default();
    control.channels[2] = 1.0;

    let state = client.exchange_data(&control).unwrap();
    assert_eq!(state, SimulatorState::default());
    drop(client);

    let requests = proxy.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].request_type, RequestType::ExchangeData);
    assert_eq!(requests[0].payload, Some(control));
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[test]
fn malformed_response_is_invalid_data() {
    let proxy = MockProxy::reply_raw(vec![0, 1, 2, 3, 4]);
    let client = RealFlightRemoteBridge::new(&proxy.addr).unwrap();

    match client.enable_rc() {
        Err(BridgeError::Connection(e)) => assert_eq!(e.kind(), ErrorKind::InvalidData),
        other => panic!("expected Connection(InvalidData), got {:?}", other),
    }
}

#[test]
fn unit_ops_fail_on_proxy_error() {
    let proxy = MockProxy::respond(Response {
        status: ResponseStatus::Error,
        payload: None,
    });
    let client = RealFlightRemoteBridge::new(&proxy.addr).unwrap();

    assert!(client.enable_rc().is_err());
    assert!(client.disable_rc().is_err());
    assert!(client.reset_aircraft().is_err());
}

#[test]
fn server_disconnect_returns_error() {
    let proxy = MockProxy::hang_up();
    let client = RealFlightRemoteBridge::new(&proxy.addr).unwrap();
    proxy.requests();

    assert!(client.enable_rc().is_err());
}

// ============================================================================
// Response / address helpers (no I/O)
// ============================================================================

mod response_helpers {
    use super::super::resolve;
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

    #[test]
    fn resolve_parses_socket_addr() {
        let addr = resolve("127.0.0.1:18083").unwrap();
        assert_eq!(addr.to_string(), "127.0.0.1:18083");
    }

    #[test]
    fn resolve_rejects_invalid_address() {
        assert!(resolve("not-a-valid-address").is_err());
    }
}
