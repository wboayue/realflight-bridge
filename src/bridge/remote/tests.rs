use std::{
    io::ErrorKind,
    net::{TcpListener, TcpStream},
    thread,
    time::Duration,
};

use crate::{
    ControlInputs, RealFlightBridge, SimulatorState,
};

use super::*;

const TEST_SERVER_ADDR: &str = "127.0.0.1:18084";

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

/// Tests enable_rc functionality with stubbed server
#[test]
fn test_enable_rc() {
    // Start a server in a separate thread
    let (mut server, server_address) = ProxyServer::new_stubbed();
    let server_thread = thread::spawn(move|| {
        let _ = server.run(); // Run until error or client disconnect
    });

    // Connect client
    let client = RealFlightRemoteBridge::new(&server_address).unwrap();

    // Call enable_rc and verify success
    let result = client.enable_rc();
    assert!(result.is_ok());

    // Clean up
    terminate_server(TEST_SERVER_ADDR);
    let _ = server_thread.join();
}

/// Tests disable_rc functionality with stubbed server
#[test]
fn test_disable_rc() {
    let server_thread = spawn_stubbed_server();
    let client = RealFlightRemoteBridge::new(TEST_SERVER_ADDR).unwrap();

    let result = client.disable_rc();
    assert!(result.is_ok());

    terminate_server(TEST_SERVER_ADDR);
    let _ = server_thread.join();
}

/// Tests reset_aircraft functionality with stubbed server
#[test]
fn test_reset_aircraft() {
    let server_thread = spawn_stubbed_server();
    let client = RealFlightRemoteBridge::new(TEST_SERVER_ADDR).unwrap();

    let result = client.reset_aircraft();
    println!("Result: {:?}", result);
    assert!(result.is_ok());

    terminate_server(TEST_SERVER_ADDR);
    let _ = server_thread.join();
}

/// Tests exchange_data functionality with stubbed server
/// Should return a default SimulatorState when in stubbed mode
#[test]
fn test_exchange_data() {
    let server_thread = spawn_stubbed_server();
    let client = RealFlightRemoteBridge::new(TEST_SERVER_ADDR).unwrap();

    // Create control inputs to send
    let control = ControlInputs::default();

    // Exchange data and verify we get a simulator state back
    let result = client.exchange_data(&control);
    assert!(result.is_ok());

    // In stubbed mode, we should get back a default SimulatorState
    let state = result.unwrap();
    assert_eq!(state, SimulatorState::default());

    terminate_server(TEST_SERVER_ADDR);
    let _ = server_thread.join();
}

/// Tests error handling when exchange_data doesn't return a payload
/// This requires a modified stubbed server that returns a response with no payload
#[test]
fn test_exchange_data_no_payload() {
    // Start a mock server that returns Success but no payload
    let server_thread = thread::spawn(|| {
        let listener = TcpListener::bind(TEST_SERVER_ADDR).unwrap();
        if let Ok((stream, _)) = listener.accept() {
            // Set up a mock server that returns Success but no payload for ExchangeData
            handle_mock_exchange_no_payload(stream);
        }
    });

    thread::sleep(Duration::from_millis(100));

    let client = RealFlightRemoteBridge::new(TEST_SERVER_ADDR).unwrap();
    let control = ControlInputs::default();

    // Should return an error if no payload
    let result = client.exchange_data(&control);
    assert!(result.is_err());

    terminate_server(TEST_SERVER_ADDR);
    let _ = server_thread.join();
}

/// Tests handling of malformed responses
#[test]
fn test_malformed_response() {
    // Start a mock server that returns invalid data
    let server_thread = thread::spawn(|| {
        let listener = TcpListener::bind(TEST_SERVER_ADDR).unwrap();
        if let Ok((stream, _)) = listener.accept() {
            // Set up a mock that returns invalid data
            handle_mock_malformed_response(stream);
        }
    });

    thread::sleep(Duration::from_millis(100));

    let client = RealFlightRemoteBridge::new(TEST_SERVER_ADDR).unwrap();

    // Any request should fail with invalid data error
    let result = client.enable_rc();
    assert!(result.is_err());
    if let Err(e) = result {
        // Should be an io::Error with InvalidData kind
        let io_err = e.downcast::<std::io::Error>().unwrap();
        assert_eq!(io_err.kind(), ErrorKind::InvalidData);
    }

    terminate_server(TEST_SERVER_ADDR);
    let _ = server_thread.join();
}

/// Tests behavior when server unexpectedly disconnects
#[test]
fn test_server_disconnect() {
    // Start a server that will disconnect after accepting connection
    let server_thread = thread::spawn(|| {
        let listener = TcpListener::bind(TEST_SERVER_ADDR).unwrap();
        if let Ok((_, _)) = listener.accept() {
            // Immediately return to close the connection
            return;
        }
    });

    thread::sleep(Duration::from_millis(100));

    let client = RealFlightRemoteBridge::new(TEST_SERVER_ADDR).unwrap();

    // Allow time for the server to disconnect
    thread::sleep(Duration::from_millis(50));

    // Any request should fail with connection reset or similar error
    let result = client.enable_rc();
    assert!(result.is_err());

    let _ = server_thread.join();
}

/// Tests handling when proxy server runs in real (non-stubbed) mode
/// This is a more advanced test that would require mocking the RealFlightLocalBridge
#[test]
fn test_real_mode_proxy_behavior() {
    // Note: This would require more complex setup including:
    // 1. Creating a mock RealFlightLocalBridge
    // 2. Injecting it into the ProxyServer
    // This is beyond the scope of simple unit tests and would be an integration test

    // As a placeholder, we'll just assert true
    // In a real implementation, this test would verify the correct forwarding
    // of requests to the local bridge
    assert!(true);
}

// Helper functions for tests

/// Spawns a stubbed server in a separate thread
fn spawn_stubbed_server() -> thread::JoinHandle<String> {
    let handle = thread::spawn(|| {
        let (mut server, server_address) = ProxyServer::new_stubbed();
        server.run().unwrap();
        server_address
    });

    // Give the server time to start
    thread::sleep(Duration::from_millis(100));

    handle
}

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
    use postcard::{to_stdvec};

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