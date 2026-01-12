use super::*;
use crate::ControlInputs;
use crate::bridge::remote::{Request, RequestType, Response, ResponseStatus};
use postcard::{from_bytes, to_stdvec};
use std::io::{Read, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

// ========================================================================
// Stub Bridge for Testing
// ========================================================================

struct StubBridge {
    enable_rc_count: Arc<AtomicUsize>,
    disable_rc_count: Arc<AtomicUsize>,
    reset_count: Arc<AtomicUsize>,
    exchange_count: Arc<AtomicUsize>,
}

impl StubBridge {
    fn new() -> Self {
        Self {
            enable_rc_count: Arc::new(AtomicUsize::new(0)),
            disable_rc_count: Arc::new(AtomicUsize::new(0)),
            reset_count: Arc::new(AtomicUsize::new(0)),
            exchange_count: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl AsyncBridge for StubBridge {
    async fn exchange_data(
        &self,
        _control: &ControlInputs,
    ) -> Result<crate::SimulatorState, BridgeError> {
        self.exchange_count.fetch_add(1, Ordering::SeqCst);
        Ok(crate::SimulatorState::default())
    }

    async fn enable_rc(&self) -> Result<(), BridgeError> {
        self.enable_rc_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    async fn disable_rc(&self) -> Result<(), BridgeError> {
        self.disable_rc_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    async fn reset_aircraft(&self) -> Result<(), BridgeError> {
        self.reset_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

// ========================================================================
// Server Creation Tests
// ========================================================================

#[tokio::test]
async fn server_binds_to_address() {
    let server = AsyncProxyServer::new("127.0.0.1:0").await;
    assert!(server.is_ok());

    let server = server.unwrap();
    assert_ne!(server.local_addr().port(), 0);
}

#[tokio::test]
async fn server_returns_local_addr() {
    let server = AsyncProxyServer::new("127.0.0.1:0").await.unwrap();
    let addr = server.local_addr();

    assert_eq!(addr.ip().to_string(), "127.0.0.1");
}

#[tokio::test]
async fn server_binds_to_any_interface() {
    let server = AsyncProxyServer::new("0.0.0.0:0").await;
    assert!(server.is_ok());
}

#[tokio::test]
async fn server_fails_on_invalid_address() {
    let result = AsyncProxyServer::new("invalid:address").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn server_fails_on_privileged_port() {
    // Port 1 requires root privileges
    let result = AsyncProxyServer::new("127.0.0.1:1").await;
    assert!(result.is_err());
}

// ========================================================================
// Cancellation Tests
// ========================================================================

#[tokio::test]
async fn shutdown_via_cancellation_token() {
    let server = AsyncProxyServer::new("127.0.0.1:0").await.unwrap();
    let cancel = CancellationToken::new();
    let bridge = StubBridge::new();

    let server_cancel = cancel.clone();
    let handle = tokio::spawn(async move { server.run_with_bridge(&bridge, server_cancel).await });

    // Give server time to start
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Cancel and wait for shutdown
    cancel.cancel();
    let result = handle.await.unwrap();
    assert!(result.is_ok());
}

// ========================================================================
// Request Routing Tests
// ========================================================================

async fn send_request_async(addr: String, request: Request) -> Response {
    tokio::task::spawn_blocking(move || {
        let mut stream = std::net::TcpStream::connect(&addr).unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();

        // Send request
        let request_bytes = to_stdvec(&request).unwrap();
        let length_bytes = (request_bytes.len() as u32).to_be_bytes();
        stream.write_all(&length_bytes).unwrap();
        stream.write_all(&request_bytes).unwrap();
        stream.flush().unwrap();

        // Read response
        let mut length_buffer = [0u8; 4];
        stream.read_exact(&mut length_buffer).unwrap();
        let response_length = u32::from_be_bytes(length_buffer) as usize;
        let mut response_buffer = vec![0u8; response_length];
        stream.read_exact(&mut response_buffer).unwrap();

        from_bytes(&response_buffer).unwrap()
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn routes_enable_rc_to_bridge() {
    let server = AsyncProxyServer::new("127.0.0.1:0").await.unwrap();
    let addr = server.local_addr().to_string();
    let cancel = CancellationToken::new();
    let bridge = StubBridge::new();
    let enable_count = bridge.enable_rc_count.clone();

    let server_cancel = cancel.clone();
    let handle = tokio::spawn(async move { server.run_with_bridge(&bridge, server_cancel).await });

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let request = Request {
        request_type: RequestType::EnableRC,
        payload: None,
    };
    let response = send_request_async(addr, request).await;

    assert!(matches!(response.status, ResponseStatus::Success));
    assert_eq!(enable_count.load(Ordering::SeqCst), 1);

    cancel.cancel();
    let _ = handle.await;
}

#[tokio::test]
async fn routes_disable_rc_to_bridge() {
    let server = AsyncProxyServer::new("127.0.0.1:0").await.unwrap();
    let addr = server.local_addr().to_string();
    let cancel = CancellationToken::new();
    let bridge = StubBridge::new();
    let disable_count = bridge.disable_rc_count.clone();

    let server_cancel = cancel.clone();
    let handle = tokio::spawn(async move { server.run_with_bridge(&bridge, server_cancel).await });

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let request = Request {
        request_type: RequestType::DisableRC,
        payload: None,
    };
    let response = send_request_async(addr, request).await;

    assert!(matches!(response.status, ResponseStatus::Success));
    assert_eq!(disable_count.load(Ordering::SeqCst), 1);

    cancel.cancel();
    let _ = handle.await;
}

#[tokio::test]
async fn routes_reset_aircraft_to_bridge() {
    let server = AsyncProxyServer::new("127.0.0.1:0").await.unwrap();
    let addr = server.local_addr().to_string();
    let cancel = CancellationToken::new();
    let bridge = StubBridge::new();
    let reset_count = bridge.reset_count.clone();

    let server_cancel = cancel.clone();
    let handle = tokio::spawn(async move { server.run_with_bridge(&bridge, server_cancel).await });

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let request = Request {
        request_type: RequestType::ResetAircraft,
        payload: None,
    };
    let response = send_request_async(addr, request).await;

    assert!(matches!(response.status, ResponseStatus::Success));
    assert_eq!(reset_count.load(Ordering::SeqCst), 1);

    cancel.cancel();
    let _ = handle.await;
}

#[tokio::test]
async fn routes_exchange_data_to_bridge() {
    let server = AsyncProxyServer::new("127.0.0.1:0").await.unwrap();
    let addr = server.local_addr().to_string();
    let cancel = CancellationToken::new();
    let bridge = StubBridge::new();
    let exchange_count = bridge.exchange_count.clone();

    let server_cancel = cancel.clone();
    let handle = tokio::spawn(async move { server.run_with_bridge(&bridge, server_cancel).await });

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let request = Request {
        request_type: RequestType::ExchangeData,
        payload: Some(ControlInputs::default()),
    };
    let response = send_request_async(addr, request).await;

    assert!(matches!(response.status, ResponseStatus::Success));
    assert!(response.payload.is_some());
    assert_eq!(exchange_count.load(Ordering::SeqCst), 1);

    cancel.cancel();
    let _ = handle.await;
}

#[tokio::test]
async fn exchange_data_without_payload_returns_error() {
    let server = AsyncProxyServer::new("127.0.0.1:0").await.unwrap();
    let addr = server.local_addr().to_string();
    let cancel = CancellationToken::new();
    let bridge = StubBridge::new();

    let server_cancel = cancel.clone();
    let handle = tokio::spawn(async move { server.run_with_bridge(&bridge, server_cancel).await });

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let request = Request {
        request_type: RequestType::ExchangeData,
        payload: None, // Missing required payload
    };
    let response = send_request_async(addr, request).await;

    assert!(matches!(response.status, ResponseStatus::Error));

    cancel.cancel();
    let _ = handle.await;
}

#[tokio::test]
async fn handles_client_disconnect() {
    let server = AsyncProxyServer::new("127.0.0.1:0").await.unwrap();
    let addr = server.local_addr().to_string();
    let cancel = CancellationToken::new();
    let bridge = StubBridge::new();

    let server_cancel = cancel.clone();
    let handle = tokio::spawn(async move { server.run_with_bridge(&bridge, server_cancel).await });

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Connect and immediately disconnect using spawn_blocking
    let addr_clone = addr.clone();
    tokio::task::spawn_blocking(move || {
        let _stream = std::net::TcpStream::connect(&addr_clone).unwrap();
        // Stream dropped here, disconnecting
    })
    .await
    .unwrap();

    // Server should still be running
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert!(!cancel.is_cancelled());

    cancel.cancel();
    let result = handle.await.unwrap();
    assert!(result.is_ok());
}

// ========================================================================
// Error Path Tests
// ========================================================================

/// Bridge that always returns errors for testing error paths.
struct FailingBridge;

impl AsyncBridge for FailingBridge {
    async fn exchange_data(
        &self,
        _control: &ControlInputs,
    ) -> Result<crate::SimulatorState, BridgeError> {
        Err(BridgeError::SoapFault("Exchange data failed".into()))
    }

    async fn enable_rc(&self) -> Result<(), BridgeError> {
        Err(BridgeError::SoapFault("Enable RC failed".into()))
    }

    async fn disable_rc(&self) -> Result<(), BridgeError> {
        Err(BridgeError::SoapFault("Disable RC failed".into()))
    }

    async fn reset_aircraft(&self) -> Result<(), BridgeError> {
        Err(BridgeError::SoapFault("Reset failed".into()))
    }
}

#[tokio::test]
async fn enable_rc_error_returns_error_response() {
    let server = AsyncProxyServer::new("127.0.0.1:0").await.unwrap();
    let addr = server.local_addr().to_string();
    let cancel = CancellationToken::new();
    let bridge = FailingBridge;

    let server_cancel = cancel.clone();
    let handle = tokio::spawn(async move { server.run_with_bridge(&bridge, server_cancel).await });

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let request = Request {
        request_type: RequestType::EnableRC,
        payload: None,
    };
    let response = send_request_async(addr, request).await;

    assert!(matches!(response.status, ResponseStatus::Error));

    cancel.cancel();
    let _ = handle.await;
}

#[tokio::test]
async fn disable_rc_error_returns_error_response() {
    let server = AsyncProxyServer::new("127.0.0.1:0").await.unwrap();
    let addr = server.local_addr().to_string();
    let cancel = CancellationToken::new();
    let bridge = FailingBridge;

    let server_cancel = cancel.clone();
    let handle = tokio::spawn(async move { server.run_with_bridge(&bridge, server_cancel).await });

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let request = Request {
        request_type: RequestType::DisableRC,
        payload: None,
    };
    let response = send_request_async(addr, request).await;

    assert!(matches!(response.status, ResponseStatus::Error));

    cancel.cancel();
    let _ = handle.await;
}

#[tokio::test]
async fn reset_aircraft_error_returns_error_response() {
    let server = AsyncProxyServer::new("127.0.0.1:0").await.unwrap();
    let addr = server.local_addr().to_string();
    let cancel = CancellationToken::new();
    let bridge = FailingBridge;

    let server_cancel = cancel.clone();
    let handle = tokio::spawn(async move { server.run_with_bridge(&bridge, server_cancel).await });

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let request = Request {
        request_type: RequestType::ResetAircraft,
        payload: None,
    };
    let response = send_request_async(addr, request).await;

    assert!(matches!(response.status, ResponseStatus::Error));

    cancel.cancel();
    let _ = handle.await;
}

#[tokio::test]
async fn exchange_data_error_returns_error_response() {
    let server = AsyncProxyServer::new("127.0.0.1:0").await.unwrap();
    let addr = server.local_addr().to_string();
    let cancel = CancellationToken::new();
    let bridge = FailingBridge;

    let server_cancel = cancel.clone();
    let handle = tokio::spawn(async move { server.run_with_bridge(&bridge, server_cancel).await });

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let request = Request {
        request_type: RequestType::ExchangeData,
        payload: Some(ControlInputs::default()),
    };
    let response = send_request_async(addr, request).await;

    assert!(matches!(response.status, ResponseStatus::Error));

    cancel.cancel();
    let _ = handle.await;
}

#[tokio::test]
async fn malformed_request_continues_handling() {
    let server = AsyncProxyServer::new("127.0.0.1:0").await.unwrap();
    let addr = server.local_addr().to_string();
    let cancel = CancellationToken::new();
    let bridge = StubBridge::new();
    let enable_count = bridge.enable_rc_count.clone();

    let server_cancel = cancel.clone();
    let handle = tokio::spawn(async move { server.run_with_bridge(&bridge, server_cancel).await });

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Send malformed data, then a valid request
    tokio::task::spawn_blocking({
        let addr = addr.clone();
        move || {
            let mut stream = std::net::TcpStream::connect(&addr).unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();

            // Send malformed data
            let garbage = vec![0xFF, 0xFF, 0xFF, 0xFF];
            let length_bytes = (garbage.len() as u32).to_be_bytes();
            stream.write_all(&length_bytes).unwrap();
            stream.write_all(&garbage).unwrap();
            stream.flush().unwrap();

            // Now send a valid request
            let request = Request {
                request_type: RequestType::EnableRC,
                payload: None,
            };
            let request_bytes = to_stdvec(&request).unwrap();
            let length_bytes = (request_bytes.len() as u32).to_be_bytes();
            stream.write_all(&length_bytes).unwrap();
            stream.write_all(&request_bytes).unwrap();
            stream.flush().unwrap();

            // Read response
            let mut length_buffer = [0u8; 4];
            stream.read_exact(&mut length_buffer).unwrap();
            let response_length = u32::from_be_bytes(length_buffer) as usize;
            let mut response_buffer = vec![0u8; response_length];
            stream.read_exact(&mut response_buffer).unwrap();

            let response: Response = from_bytes(&response_buffer).unwrap();
            response
        }
    })
    .await
    .unwrap();

    // The valid request should have been processed
    assert_eq!(enable_count.load(Ordering::SeqCst), 1);

    cancel.cancel();
    let _ = handle.await;
}
