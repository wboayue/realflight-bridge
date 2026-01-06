//! Async TCP proxy server for forwarding requests to the RealFlight simulator.

use std::net::SocketAddr;

use log::{error, info};
use postcard::{from_bytes, to_stdvec};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::sync::CancellationToken;

use crate::BridgeError;
use crate::bridge::AsyncBridge;
use crate::bridge::local::AsyncLocalBridge;

use super::super::remote::{Request, RequestType, Response};

/// Async server for forwarding requests to the RealFlight simulator.
///
/// Currently handles one client at a time (serial). Future versions may support
/// concurrent clients for multiplayer scenarios.
///
/// # Examples
///
/// ```no_run
/// use realflight_bridge::{AsyncProxyServer, BridgeError};
/// use tokio_util::sync::CancellationToken;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let server = AsyncProxyServer::new("0.0.0.0:8080").await?;
///     let cancel = CancellationToken::new();
///
///     // Spawn server
///     let server_cancel = cancel.clone();
///     let server_handle = tokio::spawn(async move {
///         server.run(server_cancel).await
///     });
///
///     // Later, trigger shutdown
///     cancel.cancel();
///     server_handle.await??;
///
///     Ok(())
/// }
/// ```
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
        info!("Async server listening on {}", self.local_addr);

        // Create the bridge to the local simulator
        let bridge = AsyncLocalBridge::new().await?;

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
                            if let Err(e) = handle_client(stream, &bridge, client_cancel).await {
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
async fn handle_client(
    stream: TcpStream,
    bridge: &AsyncLocalBridge,
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
async fn process_request(request: Request, bridge: &AsyncLocalBridge) -> Response {
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
}
