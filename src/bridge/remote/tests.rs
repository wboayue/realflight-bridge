//! Tests for RealFlightRemoteBridge and ProxyServer.
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

use crate::{BridgeError, ControlInputs, ProxyServer, RealFlightBridge, SimulatorState};

use super::{RealFlightRemoteBridge, Response, ResponseStatus};

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

/// Tests enable_rc functionality with stubbed server
#[test]
fn test_enable_rc() {
    let (mut server, server_address) = ProxyServer::new_stubbed().unwrap();

    // Start a server in a separate thread
    let server_thread = thread::spawn(move || {
        let _ = server.run(); // Run until error or client disconnect
    });

    // Connect client
    let client = RealFlightRemoteBridge::new(&server_address).unwrap();

    // Call enable_rc and verify success
    let result = client.enable_rc();

    assert!(result.is_ok(), "Enable RC failed: {:?}", result);

    let _ = server_thread.join();
}

/// Tests disable_rc functionality with stubbed server
#[test]
fn test_disable_rc() {
    let (mut server, server_address) = ProxyServer::new_stubbed().unwrap();

    // Start a server in a separate thread
    let server_thread = thread::spawn(move || {
        let _ = server.run(); // Run until error or client disconnect
    });

    // Connect client
    let client = RealFlightRemoteBridge::new(&server_address).unwrap();

    let result = client.disable_rc();

    assert!(result.is_ok(), "Disable RC failed: {:?}", result);

    let _ = server_thread.join();
}

/// Tests reset_aircraft functionality with stubbed server
#[test]
fn test_reset_aircraft() {
    let (mut server, server_address) = ProxyServer::new_stubbed().unwrap();

    // Start a server in a separate thread
    let server_thread = thread::spawn(move || {
        let _ = server.run(); // Run until error or client disconnect
    });

    // Connect client
    let client = RealFlightRemoteBridge::new(&server_address).unwrap();

    let result = client.reset_aircraft();

    assert!(result.is_ok(), "Reset aircraft failed: {:?}", result);

    let _ = server_thread.join();
}

/// Tests exchange_data functionality with stubbed server
/// Should return a default SimulatorState when in stubbed mode
#[test]
fn test_exchange_data() {
    let (mut server, server_address) = ProxyServer::new_stubbed().unwrap();

    // Start a server in a separate thread
    let server_thread = thread::spawn(move || {
        let _ = server.run(); // Run until error or client disconnect
    });

    // Connect client
    let client = RealFlightRemoteBridge::new(&server_address).unwrap();

    // Create control inputs to send
    let control = ControlInputs::default();

    // Exchange data and verify we get a simulator state back
    let result = client.exchange_data(&control);

    assert!(result.is_ok(), "Exchange data failed: {:?}", result);

    // In stubbed mode, we should get back a default SimulatorState
    let state = result.unwrap();

    assert_eq!(state, SimulatorState::default());

    let _ = server_thread.join();
}

/// Tests error handling when exchange_data doesn't return a payload
/// This requires a modified stubbed server that returns a response with no payload
#[test]
fn test_exchange_data_no_payload() {
    // Start a mock server that returns Success but no payload
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server_thread = thread::spawn(move || {
        if let Ok((stream, _)) = listener.accept() {
            // Set up a mock server that returns Success but no payload for ExchangeData
            handle_mock_exchange_no_payload(stream);
        }
    });

    thread::sleep(Duration::from_millis(100));

    let client = RealFlightRemoteBridge::new(&address.to_string()).unwrap();
    let control = ControlInputs::default();

    // Should return an error if no payload
    let result = client.exchange_data(&control);
    assert!(result.is_err());

    terminate_server(&address.to_string());
    let _ = server_thread.join();
}

/// Tests handling of malformed responses
#[test]
fn test_malformed_response() {
    // Start a mock server that returns invalid data
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server_thread = thread::spawn(move || {
        if let Ok((stream, _)) = listener.accept() {
            // Set up a mock that returns invalid data
            handle_mock_malformed_response(stream);
        }
    });

    thread::sleep(Duration::from_millis(100));

    let client = RealFlightRemoteBridge::new(&address.to_string()).unwrap();

    // Any request should fail with invalid data error
    let result = client.enable_rc();
    assert!(result.is_err());
    if let Err(e) = result {
        // Should be a Connection error with InvalidData kind
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
    // Start a server that will disconnect after accepting connection
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server_thread = thread::spawn(move || {
        if let Ok((_, _)) = listener.accept() {
            // Immediately return to close the connection
            return;
        }
    });

    thread::sleep(Duration::from_millis(100));

    let client = RealFlightRemoteBridge::new(&address.to_string()).unwrap();

    // Allow time for the server to disconnect
    thread::sleep(Duration::from_millis(50));

    // Any request should fail with connection reset or similar error
    let result = client.enable_rc();

    assert!(result.is_err());

    let _ = server_thread.join();
}

// Helper functions for tests

/// Forces termination of a running server by connecting to it
/// and then immediately closing the connection
fn terminate_server(address: &str) {
    // Connect and immediately disconnect to make the server stop accepting
    if let Ok(_) = TcpStream::connect(address) {
        // Connection successful, will drop at end of scope causing server to exit
    }
}

/// Mock handler that returns a success response with no payload for ExchangeData
fn handle_mock_exchange_no_payload(mut stream: TcpStream) {
    use postcard::to_stdvec;

    stream.set_nodelay(true).unwrap();

    let mut length_buffer = [0u8; 4];
    let _ = stream.read_exact(&mut length_buffer);

    let msg_length = u32::from_be_bytes(length_buffer) as usize;
    let mut buffer = vec![0u8; msg_length];
    let _ = stream.read_exact(&mut buffer);

    // Create a success response with no payload
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

    // Send invalid data
    let malformed_data = vec![0, 1, 2, 3, 4];
    let length_bytes = (malformed_data.len() as u32).to_be_bytes();

    let _ = stream.write_all(&length_bytes);
    let _ = stream.write_all(&malformed_data.as_slice());
    let _ = stream.flush();
}
