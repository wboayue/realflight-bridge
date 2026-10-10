//! TCP proxy server for forwarding requests to the RealFlight simulator.
//!
//! The proxy server listens for client connections and forwards requests to the
//! local RealFlight simulator. This module requires the `rt-tokio` feature.

#![cfg(feature = "rt-tokio")]

mod handler;

#[cfg(test)]
mod tests;

use std::net::SocketAddr;
use std::time::Duration;

use log::{error, info};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::sync::CancellationToken;

use crate::BridgeError;
use crate::bridge::AsyncBridge;
use crate::bridge::local::AsyncLocalBridge;

use handler::handle_client;

/// Async server for forwarding requests to the RealFlight simulator.
///
/// Serves one client at a time. A new connection replaces the current client,
/// so a client that reconnects (e.g. after a timeout) is never blocked by its
/// stale connection.
pub struct AsyncProxyServer {
    listener: TcpListener,
    local_addr: SocketAddr,
    simulator_addr: SocketAddr,
    preconnect: bool,
    request_timeout: Duration,
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
            simulator_addr: crate::DEFAULT_SIMULATOR_HOST.parse().unwrap(),
            preconnect: false,
            request_timeout: crate::defaults::REQUEST_TIMEOUT,
        })
    }

    /// Sets the simulator address requests are forwarded to
    /// (default [`DEFAULT_SIMULATOR_HOST`](crate::DEFAULT_SIMULATOR_HOST)).
    #[must_use]
    pub fn simulator_addr(mut self, addr: SocketAddr) -> Self {
        self.simulator_addr = addr;
        self
    }

    /// Sets whether the next simulator connection is opened ahead of each request
    /// (default `false`: each request opens its own connection).
    ///
    /// Pre-connecting hides connect latency but holds one idle connection open to
    /// the simulator, which newer RealFlight versions stall on. Only enable it for
    /// versions that tolerate idle connections.
    #[must_use]
    pub fn preconnect(mut self, preconnect: bool) -> Self {
        self.preconnect = preconnect;
        self
    }

    /// Sets the time allowed for each simulator request (default 50 ms). A request
    /// that takes longer is answered with an error, so the client isn't left
    /// waiting. Keep it below the client's I/O timeout. Must be non-zero; a
    /// zero timeout fails each client's session at startup.
    #[must_use]
    pub fn request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }

    /// Returns the local address the server is bound to.
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Runs the server until the cancellation token is triggered.
    ///
    /// The simulator isn't contacted until a client connects, and all simulator
    /// connections are closed when it disconnects. By default each request opens its
    /// own connection; with [`preconnect`](Self::preconnect) one connection is held
    /// open ahead of use while a client is connected.
    ///
    /// A new connection replaces the current client: the old one is
    /// disconnected, and its simulator connections closed, before the new one is
    /// served.
    ///
    /// # Arguments
    /// * `cancel` - Cancellation token for graceful shutdown.
    ///
    /// # Returns
    /// A `Result` indicating success or an error.
    pub async fn run(&self, cancel: CancellationToken) -> Result<(), BridgeError> {
        let pool_size = if self.preconnect { 1 } else { 0 };
        let builder = AsyncLocalBridge::builder()
            .addr(self.simulator_addr)
            .pool_size(pool_size)
            .request_timeout(self.request_timeout);
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

    /// Accepts clients until cancelled, serving one at a time with `on_client`.
    ///
    /// A new connection replaces the current client: its session is cancelled
    /// (an in-flight request finishes first) and awaited before the new one
    /// starts. A client that timed out and reconnects without closing its old
    /// socket would otherwise be stuck behind it.
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

        let mut next: Option<(TcpStream, SocketAddr)> = None;
        loop {
            let (stream, addr) = match next.take() {
                Some(client) => client,
                None => tokio::select! {
                    _ = cancel.cancelled() => break,
                    result = self.listener.accept() => match result {
                        Ok(client) => client,
                        Err(e) => {
                            error!("Failed to accept connection: {}", e);
                            continue;
                        }
                    },
                },
            };

            info!("New client connected: {}", addr);
            let client_cancel = cancel.child_token();
            let client = on_client(stream, client_cancel.clone());
            tokio::pin!(client);

            let result = loop {
                tokio::select! {
                    biased;
                    result = &mut client => break result,
                    // Parent cancellation propagates to the client's child token
                    _ = cancel.cancelled() => break client.await,
                    result = self.listener.accept() => match result {
                        Ok((stream, new_addr)) => {
                            info!("Replacing client {} with {}", addr, new_addr);
                            client_cancel.cancel();
                            next = Some((stream, new_addr));
                            break client.await;
                        }
                        Err(e) => error!("Failed to accept connection: {}", e),
                    },
                }
            };
            if let Err(e) = result {
                error!("Error handling client: {}", e);
            }
            if cancel.is_cancelled() {
                break;
            }
        }

        info!("Server shutdown requested");
        Ok(())
    }
}
