//! TCP proxy server for forwarding requests to the RealFlight simulator.
//!
//! The proxy server listens for client connections and forwards requests to the
//! local RealFlight simulator. This module requires the `rt-tokio` feature.

#![cfg(feature = "rt-tokio")]

use std::net::SocketAddr;

use log::{error, info};
use postcard::{from_bytes, to_stdvec};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::sync::CancellationToken;

use crate::BridgeError;
use crate::bridge::AsyncBridge;
use crate::bridge::local::AsyncLocalBridge;
use crate::bridge::remote::{Request, RequestType, Response};

/// Async server for forwarding requests to the RealFlight simulator.
///
/// Currently handles one client at a time (serial). Future versions may support
/// concurrent clients for multiplayer scenarios.
pub struct AsyncProxyServer {
    listener: TcpListener,
    local_addr: SocketAddr,
}

impl AsyncProxyServer {
    /// Creates a new async server instance with a connection to the local simulator.
    ///
    /// # Arguments
    /// * `bind_address` - The address to bind to (e.g., "0.0.0.0:8080").
    ///
    /// # Returns
    /// A `Result` containing the server instance or an error if binding fails.
    pub async fn new(bind_address: &str) -> Result<Self, BridgeError> {
        let listener = TcpListener::bind(bind_address).await?;
        let local_addr = listener.local_addr()?;

        Ok(AsyncProxyServer {
            listener,
            local_addr,
        })
    }

    /// Returns the local address the server is bound to.
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Runs the server until the cancellation token is triggered.
    ///
    /// # Arguments
    /// * `cancel` - Cancellation token for graceful shutdown.
    ///
    /// # Returns
    /// A `Result` indicating success or an error.
    pub async fn run(&self, cancel: CancellationToken) -> Result<(), BridgeError> {
        let bridge = AsyncLocalBridge::new().await?;
        self.run_with_bridge(&bridge, cancel).await
    }

    /// Runs the server with a custom bridge implementation.
    ///
    /// This is useful for testing with mock bridges.
    pub async fn run_with_bridge<B: AsyncBridge>(
        &self,
        bridge: &B,
        cancel: CancellationToken,
    ) -> Result<(), BridgeError> {
        info!("Async server listening on {}", self.local_addr);

        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    info!("Server shutdown requested");
                    break;
                }
                result = self.listener.accept() => {
                    match result {
                        Ok((stream, addr)) => {
                            info!("New client connected: {}", addr);
                            let client_cancel = cancel.clone();
                            // For now, handle clients serially like the sync version
                            // Could be changed to spawn tasks for concurrent clients
                            if let Err(e) = handle_client(stream, bridge, client_cancel).await {
                                error!("Error handling client: {}", e);
                            }
                        }
                        Err(e) => {
                            error!("Failed to accept connection: {}", e);
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

/// Handles a single client connection.
async fn handle_client<B: AsyncBridge>(
    stream: TcpStream,
    bridge: &B,
    cancel: CancellationToken,
) -> Result<(), BridgeError> {
    stream.set_nodelay(true)?;

    let (read_half, write_half) = stream.into_split();
    let mut reader = BufReader::new(read_half);
    let mut writer = BufWriter::new(write_half);
    let mut length_buffer = [0u8; 4];

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                break;
            }
            result = reader.read_exact(&mut length_buffer) => {
                if result.is_err() {
                    break; // Client disconnected
                }

                let msg_length = u32::from_be_bytes(length_buffer) as usize;

                // Read the request data
                let mut buffer = vec![0u8; msg_length];
                reader.read_exact(&mut buffer).await?;

                // Deserialize the request
                let request: Request = match from_bytes(&buffer) {
                    Ok(req) => req,
                    Err(e) => {
                        error!("Failed to deserialize request: {}", e);
                        continue;
                    }
                };

                // Process request
                let response = process_request(request, bridge).await;
                send_response(&mut writer, response).await?;
            }
        }
    }

    info!("Client disconnected");
    Ok(())
}

/// Sends a response to the client.
async fn send_response(
    writer: &mut BufWriter<tokio::net::tcp::OwnedWriteHalf>,
    response: Response,
) -> Result<(), BridgeError> {
    let response_bytes = to_stdvec(&response)
        .map_err(|e| BridgeError::SoapFault(format!("Failed to serialize response: {}", e)))?;
    let length_bytes = (response_bytes.len() as u32).to_be_bytes();

    writer.write_all(&length_bytes).await?;
    writer.write_all(&response_bytes).await?;
    writer.flush().await?;

    Ok(())
}

/// Processes a request using the async bridge.
async fn process_request<B: AsyncBridge>(request: Request, bridge: &B) -> Response {
    match request.request_type {
        RequestType::EnableRC => match bridge.enable_rc().await {
            Ok(()) => Response::success(),
            Err(e) => {
                error!("Error enabling RC: {}", e);
                Response::error()
            }
        },
        RequestType::DisableRC => match bridge.disable_rc().await {
            Ok(()) => Response::success(),
            Err(e) => {
                error!("Error disabling RC: {}", e);
                Response::error()
            }
        },
        RequestType::ResetAircraft => match bridge.reset_aircraft().await {
            Ok(()) => Response::success(),
            Err(e) => {
                error!("Error resetting aircraft: {}", e);
                Response::error()
            }
        },
        RequestType::ExchangeData => match request.payload {
            Some(payload) => match bridge.exchange_data(&payload).await {
                Ok(state) => Response::success_with(state),
                Err(e) => {
                    error!("Error exchanging data: {}", e);
                    Response::error()
                }
            },
            None => Response::error(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ControlInputs;
    use crate::bridge::remote::ResponseStatus;
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
        let handle =
            tokio::spawn(async move { server.run_with_bridge(&bridge, server_cancel).await });

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
        let handle =
            tokio::spawn(async move { server.run_with_bridge(&bridge, server_cancel).await });

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
        let handle =
            tokio::spawn(async move { server.run_with_bridge(&bridge, server_cancel).await });

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
        let handle =
            tokio::spawn(async move { server.run_with_bridge(&bridge, server_cancel).await });

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
        let handle =
            tokio::spawn(async move { server.run_with_bridge(&bridge, server_cancel).await });

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
        let handle =
            tokio::spawn(async move { server.run_with_bridge(&bridge, server_cancel).await });

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
        let handle =
            tokio::spawn(async move { server.run_with_bridge(&bridge, server_cancel).await });

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
}
