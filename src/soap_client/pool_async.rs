//! Async connection pool for TCP connections to the RealFlight simulator.
//!
//! The RealFlight SoapServer requires a new connection for each request.
//! This pool pre-creates connections in the background to hide latency.

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use log::{debug, error};
use tokio::net::TcpStream;
use tokio::sync::{Mutex, mpsc};
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
    initialized: Arc<AtomicBool>,
    statistics: Arc<StatisticsEngine>,
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

        let initialized = Arc::new(AtomicBool::new(false));
        let init_clone = Arc::clone(&initialized);
        let stats_clone = Arc::clone(&statistics);
        let task_cancel = cancel.clone();

        debug!("Creating {} async connections in pool.", pool_size);

        // Spawn background task to create connections
        tokio::spawn(async move {
            // Create initial connections
            for i in 0..pool_size {
                match timeout(connect_timeout, TcpStream::connect(addr)).await {
                    Ok(Ok(stream)) => {
                        if tx.send(stream).await.is_err() {
                            error!("Failed to queue initial connection {}", i);
                            return;
                        }
                    }
                    Ok(Err(e)) => {
                        error!("Failed to connect to simulator at {}: {}", addr, e);
                        return;
                    }
                    Err(_) => {
                        error!("Connection timeout to simulator at {}", addr);
                        return;
                    }
                }
            }

            init_clone.store(true, Ordering::Release);

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
                                stats_clone.increment_error_count();
                                tokio::time::sleep(connect_timeout).await;
                            }
                            Err(_) => {
                                error!("Connection timeout");
                                stats_clone.increment_error_count();
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
            initialized,
            statistics,
        })
    }

    /// Waits for the pool to be initialized with initial connections.
    pub async fn ensure_initialized(&self, init_timeout: Duration) -> Result<(), BridgeError> {
        let start = std::time::Instant::now();
        while !self.initialized.load(Ordering::Acquire) {
            if start.elapsed() > init_timeout {
                return Err(BridgeError::Initialization(format!(
                    "Connection pool did not initialize. Waited for {:?}.",
                    init_timeout
                )));
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        Ok(())
    }

    /// Gets a connection from the pool.
    pub async fn get_connection(&self) -> Result<TcpStream, BridgeError> {
        let mut rx = self.connections.lock().await;
        rx.recv().await.ok_or_else(|| {
            BridgeError::Initialization("Connection pool closed".into())
        })
    }

    /// Returns a reference to the statistics engine.
    pub fn statistics(&self) -> &Arc<StatisticsEngine> {
        &self.statistics
    }
}

impl Drop for AsyncConnectionPool {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener as TokioTcpListener;

    #[tokio::test]
    async fn pool_creates_connections() {
        let listener = TokioTcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let stats = Arc::new(StatisticsEngine::new());

        // Accept connections in background using async listener
        let accept_handle = tokio::spawn(async move {
            for _ in 0..3 {
                let _ = listener.accept().await;
            }
        });

        let pool = AsyncConnectionPool::new(
            addr,
            Duration::from_secs(1),
            2,
            stats,
        )
        .await
        .unwrap();

        pool.ensure_initialized(Duration::from_secs(5)).await.unwrap();

        let conn = pool.get_connection().await;
        assert!(conn.is_ok());

        drop(pool);
        let _ = accept_handle.await;
    }

    #[tokio::test]
    async fn pool_fails_on_invalid_address() {
        let stats = Arc::new(StatisticsEngine::new());

        // Use a port that's unlikely to be listening
        let addr: SocketAddr = "127.0.0.1:1".parse().unwrap();

        let pool = AsyncConnectionPool::new(
            addr,
            Duration::from_millis(100),
            1,
            stats,
        )
        .await
        .unwrap();

        // Should timeout waiting for initialization
        let result = pool.ensure_initialized(Duration::from_millis(500)).await;
        assert!(result.is_err());
    }
}
