//! Async connection pool for TCP connections to the RealFlight simulator.
//!
//! The RealFlight SoapServer requires a new connection for each request.
//! This pool pre-creates connections in the background to hide latency.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use log::{debug, error};
use tokio::net::TcpStream;
use tokio::sync::{Mutex, mpsc, watch};
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

use crate::BridgeError;
use crate::StatisticsEngine;

/// Pre-creates TCP connections in a background task to hide connection latency.
///
/// The RealFlight SoapServer requires a new connection for each request.
/// The idea for the pool is to create the next connection
/// in the background while the current request is being processed.
///
/// # Cancellation Safety
///
/// `get_connection()` is **NOT cancel-safe**. If the future is dropped while
/// awaiting `recv()`, a connection may be lost from the channel. This is an
/// acceptable trade-off because:
/// - The background task continuously creates replacement connections
/// - Cancellation mid-receive is rare in practice (typically only on shutdown)
/// - The pool self-heals within one connection-creation cycle
///
/// If cancel-safety is required, wrap calls in `tokio::select!` with care or
/// use a dedicated cancellation token rather than dropping the future.
pub(crate) struct AsyncConnectionPool {
    connections: Mutex<mpsc::Receiver<TcpStream>>,
    cancel: CancellationToken,
    init_result: watch::Receiver<Option<Result<(), String>>>,
}

impl AsyncConnectionPool {
    /// Creates a new async connection pool.
    ///
    /// # Arguments
    /// * `addr` - The address to connect to
    /// * `connect_timeout` - Timeout for establishing connections
    /// * `pool_size` - Number of connections to pre-create
    /// * `statistics` - Statistics engine for tracking errors
    pub async fn new(
        addr: SocketAddr,
        connect_timeout: Duration,
        pool_size: usize,
        statistics: Arc<StatisticsEngine>,
    ) -> Result<Self, BridgeError> {
        let cancel = CancellationToken::new();
        let (tx, rx) = mpsc::channel(pool_size);

        // Channel for communicating initialization result
        let (init_tx, init_rx) = watch::channel(None);
        let task_cancel = cancel.clone();

        debug!("Creating {} async connections in pool.", pool_size);

        // Spawn background task to create connections
        tokio::spawn(async move {
            // Create initial connections
            for i in 0..pool_size {
                match timeout(connect_timeout, TcpStream::connect(addr)).await {
                    Ok(Ok(stream)) => {
                        if tx.send(stream).await.is_err() {
                            let msg = format!("Failed to queue initial connection {}", i);
                            error!("{}", msg);
                            let _ = init_tx.send(Some(Err(msg)));
                            return;
                        }
                    }
                    Ok(Err(e)) => {
                        let msg = format!("Failed to connect to simulator at {}: {}", addr, e);
                        error!("{}", msg);
                        let _ = init_tx.send(Some(Err(msg)));
                        return;
                    }
                    Err(_) => {
                        let msg = format!("Connection timeout to simulator at {}", addr);
                        error!("{}", msg);
                        let _ = init_tx.send(Some(Err(msg)));
                        return;
                    }
                }
            }

            let _ = init_tx.send(Some(Ok(())));

            // Continue creating connections as needed
            loop {
                tokio::select! {
                    _ = task_cancel.cancelled() => {
                        debug!("Connection pool shutting down");
                        break;
                    }
                    result = timeout(connect_timeout, TcpStream::connect(addr)) => {
                        match result {
                            Ok(Ok(stream)) => {
                                if tx.send(stream).await.is_err() {
                                    break; // Receiver dropped
                                }
                            }
                            Ok(Err(e)) => {
                                error!("Error creating connection: {}", e);
                                statistics.increment_error_count();
                                tokio::time::sleep(connect_timeout).await;
                            }
                            Err(_) => {
                                error!("Connection timeout");
                                statistics.increment_error_count();
                                tokio::time::sleep(connect_timeout).await;
                            }
                        }
                    }
                }
            }
        });

        Ok(Self {
            connections: Mutex::new(rx),
            cancel,
            init_result: init_rx,
        })
    }

    /// Waits for the pool to be initialized with initial connections.
    pub async fn ensure_initialized(&self, init_timeout: Duration) -> Result<(), BridgeError> {
        let mut rx = self.init_result.clone();
        let start = std::time::Instant::now();

        loop {
            // Check current value
            if let Some(result) = rx.borrow().as_ref() {
                return match result {
                    Ok(()) => Ok(()),
                    Err(msg) => Err(BridgeError::Initialization(msg.clone())),
                };
            }

            // Check timeout
            let remaining = init_timeout.saturating_sub(start.elapsed());
            if remaining.is_zero() {
                return Err(BridgeError::Initialization(format!(
                    "Connection pool did not initialize. Waited for {:?}.",
                    init_timeout
                )));
            }

            // Wait for change with timeout
            match timeout(remaining, rx.changed()).await {
                Ok(Ok(())) => continue, // Value changed, check again
                Ok(Err(_)) => {
                    return Err(BridgeError::Initialization(
                        "Initialization channel closed unexpectedly".into(),
                    ));
                }
                Err(_) => {
                    return Err(BridgeError::Initialization(format!(
                        "Connection pool did not initialize. Waited for {:?}.",
                        init_timeout
                    )));
                }
            }
        }
    }

    /// Gets a connection from the pool.
    pub async fn get_connection(&self) -> Result<TcpStream, BridgeError> {
        let mut rx = self.connections.lock().await;
        rx.recv()
            .await
            .ok_or_else(|| BridgeError::Initialization("Connection pool closed".into()))
    }
}

impl Drop for AsyncConnectionPool {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

#[cfg(test)]
mod tests;
