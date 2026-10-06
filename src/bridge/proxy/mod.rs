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
    preconnect: bool,
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
            preconnect: true,
        })
    }

    /// Sets whether the next simulator connection is opened ahead of each request
    /// (default `true`).
    ///
    /// Pre-connecting hides connect latency but holds one idle connection open to
    /// the simulator, which some RealFlight versions stall on. With `false`, each
    /// request opens its own connection.
    #[must_use]
    pub fn preconnect(mut self, preconnect: bool) -> Self {
        self.preconnect = preconnect;
        self
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
        let mut builder = AsyncLocalBridge::builder();
        if !self.preconnect {
            builder = builder.pool_size(0);
        }
        self.serve(cancel, |stream, cancel| {
            let builder = builder.clone();
            async move {
                let bridge = builder.build().await?;
                handle_client(stream, &bridge, cancel).await
            }
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
