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
use tokio::net::TcpListener;
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
