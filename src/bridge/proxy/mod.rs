//! TCP proxy server for forwarding requests to the RealFlight simulator.
//!
//! The proxy server listens for client connections and forwards requests to the
//! local RealFlight simulator. This module requires the `rt-tokio` feature.

#![cfg(feature = "rt-tokio")]

mod handler;

#[cfg(test)]
mod tests;

use std::net::SocketAddr;

use log::{error, info};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::sync::CancellationToken;

use crate::BridgeError;
use crate::bridge::AsyncBridge;
use crate::bridge::local::AsyncLocalBridge;

use handler::handle_client;

/// Async server for forwarding requests to the RealFlight simulator.
///
/// Currently handles one client at a time (serial). Future versions may support
/// concurrent clients for multiplayer scenarios.
pub struct AsyncProxyServer {
    listener: TcpListener,
    local_addr: SocketAddr,
}

impl AsyncProxyServer {
    /// Creates a new async server bound to `bind_address`.
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
    /// A simulator connection is opened when a client connects and closed when it
    /// disconnects, so the proxy holds no idle connections to the simulator.
    ///
    /// # Arguments
    /// * `cancel` - Cancellation token for graceful shutdown.
    ///
    /// # Returns
    /// A `Result` indicating success or an error.
    pub async fn run(&self, cancel: CancellationToken) -> Result<(), BridgeError> {
        self.serve(cancel, |stream, cancel| async move {
            let bridge = AsyncLocalBridge::new().await?;
            handle_client(stream, &bridge, cancel).await
        })
        .await
    }

    /// Runs the server with a custom bridge implementation.
    ///
    /// This is useful for testing with mock bridges.
    pub async fn run_with_bridge<B: AsyncBridge>(
        &self,
        bridge: &B,
        cancel: CancellationToken,
    ) -> Result<(), BridgeError> {
        self.serve(cancel, |stream, cancel| {
            handle_client(stream, bridge, cancel)
        })
        .await
    }

    /// Accepts clients until cancelled, handling each serially with `on_client`.
    async fn serve<F, Fut>(
        &self,
        cancel: CancellationToken,
        on_client: F,
    ) -> Result<(), BridgeError>
    where
        F: Fn(TcpStream, CancellationToken) -> Fut,
        Fut: Future<Output = Result<(), BridgeError>>,
    {
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
                            // Clients are handled serially; could spawn tasks for concurrent clients
                            if let Err(e) = on_client(stream, cancel.clone()).await {
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
