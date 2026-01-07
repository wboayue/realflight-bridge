//! Tests for RealFlightRemoteBridge.
//!
//! Organized into submodules:
//! - `connection_tests`: Connection establishment and timeout tests
//! - `operation_tests`: Tests for bridge operations (enable_rc, disable_rc, etc.)
//! - `error_handling`: Tests for error conditions and edge cases

use std::{
    io::{ErrorKind, Read, Write},
    net::{TcpListener, TcpStream},
    thread,
    time::Duration,
};

use postcard::{from_bytes, to_stdvec};

use crate::{BridgeError, ControlInputs, RealFlightBridge, SimulatorState};

use super::{RealFlightRemoteBridge, Request, RequestType, Response, ResponseStatus};

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

/// Tests enable_rc functionality with mock server
#[test]
fn test_enable_rc() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();

    let server_thread = thread::spawn(move || {
        if let Ok((stream, _)) = listener.accept() {
            handle_mock_success(stream);
        }
    });

    thread::sleep(Duration::from_millis(50));

    let client = RealFlightRemoteBridge::new(&address.to_string()).unwrap();
    let result = client.enable_rc();

    assert!(result.is_ok(), "Enable RC failed: {:?}", result);

    let _ = server_thread.join();
}

/// Tests disable_rc functionality with mock server
#[test]
fn test_disable_rc() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();

    let server_thread = thread::spawn(move || {
        if let Ok((stream, _)) = listener.accept() {
            handle_mock_success(stream);
        }
    });

    thread::sleep(Duration::from_millis(50));

    let client = RealFlightRemoteBridge::new(&address.to_string()).unwrap();
    let result = client.disable_rc();

    assert!(result.is_ok(), "Disable RC failed: {:?}", result);

    let _ = server_thread.join();
}

/// Tests reset_aircraft functionality with mock server
#[test]
fn test_reset_aircraft() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();

    let server_thread = thread::spawn(move || {
        if let Ok((stream, _)) = listener.accept() {
            handle_mock_success(stream);
        }
    });

    thread::sleep(Duration::from_millis(50));

    let client = RealFlightRemoteBridge::new(&address.to_string()).unwrap();
    let result = client.reset_aircraft();

    assert!(result.is_ok(), "Reset aircraft failed: {:?}", result);

    let _ = server_thread.join();
}

/// Tests exchange_data functionality with mock server
#[test]
fn test_exchange_data() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();

    let server_thread = thread::spawn(move || {
        if let Ok((stream, _)) = listener.accept() {
            handle_mock_exchange_data(stream);
        }
    });

    thread::sleep(Duration::from_millis(50));

    let client = RealFlightRemoteBridge::new(&address.to_string()).unwrap();
    let control = ControlInputs::default();

    let result = client.exchange_data(&control);

    assert!(result.is_ok(), "Exchange data failed: {:?}", result);

    let state = result.unwrap();
    assert_eq!(state, SimulatorState::default());

    let _ = server_thread.join();
}

/// Tests error handling when exchange_data doesn't return a payload
#[test]
fn test_exchange_data_no_payload() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server_thread = thread::spawn(move || {
        if let Ok((stream, _)) = listener.accept() {
            handle_mock_exchange_no_payload(stream);
        }
    });

    thread::sleep(Duration::from_millis(100));

    let client = RealFlightRemoteBridge::new(&address.to_string()).unwrap();
    let control = ControlInputs::default();

    let result = client.exchange_data(&control);
    assert!(result.is_err());

    terminate_server(&address.to_string());
    let _ = server_thread.join();
}

/// Tests handling of malformed responses
#[test]
fn test_malformed_response() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server_thread = thread::spawn(move || {
        if let Ok((stream, _)) = listener.accept() {
            handle_mock_malformed_response(stream);
        }
    });

    thread::sleep(Duration::from_millis(100));

    let client = RealFlightRemoteBridge::new(&address.to_string()).unwrap();

    let result = client.enable_rc();
    assert!(result.is_err());
    if let Err(e) = result {
        match e {
            BridgeError::Connection(io_err) => {
                assert_eq!(io_err.kind(), ErrorKind::InvalidData);
            }
            _ => panic!("Expected BridgeError::Connection, got {:?}", e),
        }
    }

    terminate_server(&address.to_string());
    let _ = server_thread.join();
}

/// Tests behavior when server unexpectedly disconnects
#[test]
fn test_server_disconnect() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server_thread = thread::spawn(move || {
        if let Ok((_, _)) = listener.accept() {
            return;
        }
    });

    thread::sleep(Duration::from_millis(100));

    let client = RealFlightRemoteBridge::new(&address.to_string()).unwrap();

    thread::sleep(Duration::from_millis(50));

    let result = client.enable_rc();

    assert!(result.is_err());

    let _ = server_thread.join();
}

// ============================================================================
// Helper functions
// ============================================================================

/// Forces termination of a running server by connecting to it
fn terminate_server(address: &str) {
    if let Ok(_) = TcpStream::connect(address) {}
}

/// Mock handler that returns a success response
fn handle_mock_success(mut stream: TcpStream) {
    stream.set_nodelay(true).unwrap();

    let mut length_buffer = [0u8; 4];
    let _ = stream.read_exact(&mut length_buffer);

    let msg_length = u32::from_be_bytes(length_buffer) as usize;
    let mut buffer = vec![0u8; msg_length];
    let _ = stream.read_exact(&mut buffer);

    let response = Response {
        status: ResponseStatus::Success,
        payload: None,
    };

    let response_bytes = to_stdvec(&response).unwrap();
    let length_bytes = (response_bytes.len() as u32).to_be_bytes();

    let _ = stream.write_all(&length_bytes);
    let _ = stream.write_all(&response_bytes);
    let _ = stream.flush();
}

/// Mock handler that returns a success response with SimulatorState payload
fn handle_mock_exchange_data(mut stream: TcpStream) {
    stream.set_nodelay(true).unwrap();

    let mut length_buffer = [0u8; 4];
    let _ = stream.read_exact(&mut length_buffer);

    let msg_length = u32::from_be_bytes(length_buffer) as usize;
    let mut buffer = vec![0u8; msg_length];
    let _ = stream.read_exact(&mut buffer);

    // Verify request type
    let request: Request = from_bytes(&buffer).unwrap();
    assert_eq!(request.request_type, RequestType::ExchangeData);

    let response = Response {
        status: ResponseStatus::Success,
        payload: Some(SimulatorState::default()),
    };

    let response_bytes = to_stdvec(&response).unwrap();
    let length_bytes = (response_bytes.len() as u32).to_be_bytes();

    let _ = stream.write_all(&length_bytes);
    let _ = stream.write_all(&response_bytes);
    let _ = stream.flush();
}

/// Mock handler that returns a success response with no payload for ExchangeData
fn handle_mock_exchange_no_payload(mut stream: TcpStream) {
    stream.set_nodelay(true).unwrap();

    let mut length_buffer = [0u8; 4];
    let _ = stream.read_exact(&mut length_buffer);

    let msg_length = u32::from_be_bytes(length_buffer) as usize;
    let mut buffer = vec![0u8; msg_length];
    let _ = stream.read_exact(&mut buffer);

    let response = Response {
        status: ResponseStatus::Success,
        payload: None,
    };

    let response_bytes = to_stdvec(&response).unwrap();
    let length_bytes = (response_bytes.len() as u32).to_be_bytes();

    let _ = stream.write_all(&length_bytes);
    let _ = stream.write_all(&response_bytes);
    let _ = stream.flush();
}

/// Mock handler that returns malformed data
fn handle_mock_malformed_response(mut stream: TcpStream) {
    stream.set_nodelay(true).unwrap();

    let mut length_buffer = [0u8; 4];
    let _ = stream.read_exact(&mut length_buffer);

    let msg_length = u32::from_be_bytes(length_buffer) as usize;
    let mut buffer = vec![0u8; msg_length];
    let _ = stream.read_exact(&mut buffer);

    let malformed_data = vec![0, 1, 2, 3, 4];
    let length_bytes = (malformed_data.len() as u32).to_be_bytes();

    let _ = stream.write_all(&length_bytes);
    let _ = stream.write_all(&malformed_data.as_slice());
    let _ = stream.flush();
}
