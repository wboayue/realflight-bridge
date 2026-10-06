//! Tests for RealFlightRemoteBridge.
//!
//! Organized into submodules:
//! - `connection_tests`: Connection establishment and timeout tests
//! - `operation_tests`: Tests for bridge operations (enable_rc, disable_rc, etc.)
//! - `error_handling`: Tests for error conditions and edge cases

use std::{io::ErrorKind, time::Duration};

use crate::{BridgeError, ControlInputs, RealFlightBridge, SimulatorState};

use super::{RealFlightRemoteBridge, RemoteError, RequestType, Response, ResponseStatus};
use crate::bridge::wire::test_support::MockProxy;

// ============================================================================
// Connection Tests
// ============================================================================

/// Tests connecting to a non-existent server - should fail with connection refused
#[test]
fn connection_failure() {
    // Attempt to connect to a port where no server is running
    let result = RealFlightRemoteBridge::new("127.0.0.1:1");

    assert!(result.is_err());
    if let Err(e) = result {
        assert_eq!(e.kind(), ErrorKind::ConnectionRefused);
    }
}

/// Tests custom timeout functionality
#[test]
fn with_timeout_connection_failure() {
    let start = std::time::Instant::now();
    let result = RealFlightRemoteBridge::with_timeout("127.0.0.1:1", Duration::from_millis(100));
    let elapsed = start.elapsed();

    assert!(result.is_err());
    // Should fail relatively quickly (within reasonable margin of timeout)
    assert!(elapsed < Duration::from_secs(2));
}

/// Tests invalid address handling
#[test]
fn invalid_address() {
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
fn oversized_response_is_invalid_data() {
    let proxy = MockProxy::reply_header(u32::MAX);
    let client = RealFlightRemoteBridge::new(&proxy.addr).unwrap();

    match client.enable_rc() {
        Err(BridgeError::Connection(e)) => assert_eq!(e.kind(), ErrorKind::InvalidData),
        other => panic!("expected Connection(InvalidData), got {:?}", other),
    }
}

#[test]
fn failed_call_marks_connection_out_of_sync() {
    let proxy = MockProxy::reply_header(u32::MAX);
    let client = RealFlightRemoteBridge::new(&proxy.addr).unwrap();
    assert!(client.enable_rc().is_err());

    match client.enable_rc() {
        Err(BridgeError::Connection(e)) => assert!(e.to_string().contains("out of sync")),
        other => panic!("expected Connection error, got {:?}", other),
    }
}

#[test]
fn unit_ops_fail_on_proxy_error() {
    let proxy = MockProxy::respond(Response {
        status: ResponseStatus::Error(RemoteError::Protocol("bad".into())),
        payload: None,
    });
    let client = RealFlightRemoteBridge::new(&proxy.addr).unwrap();

    assert!(client.enable_rc().is_err());
    assert!(client.disable_rc().is_err());
    assert!(client.reset_aircraft().is_err());
}

#[test]
fn relays_simulator_fault_from_proxy() {
    let proxy = MockProxy::respond(Response {
        status: ResponseStatus::Error(RemoteError::SoapFault(
            "Preexisting controller reference".into(),
        )),
        payload: None,
    });
    let client = RealFlightRemoteBridge::new(&proxy.addr).unwrap();

    match client.disable_rc() {
        Err(BridgeError::SoapFault(msg)) => assert_eq!(msg, "Preexisting controller reference"),
        other => panic!("expected SoapFault, got {:?}", other),
    }
}

#[test]
fn server_disconnect_returns_error() {
    let proxy = MockProxy::hang_up();
    let client = RealFlightRemoteBridge::new(&proxy.addr).unwrap();
    proxy.requests();

    assert!(client.enable_rc().is_err());
}

// ============================================================================
// Address resolution (no I/O)
// ============================================================================

mod resolve {
    use super::super::resolve;

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
