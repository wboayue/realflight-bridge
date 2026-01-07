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

        // Channel for communicating initialization result
        let (init_tx, init_rx) = watch::channel(None);
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
            init_result: init_rx,
            statistics,
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
    use tokio::io::AsyncWriteExt;
    use tokio::net::TcpListener as TokioTcpListener;

    #[tokio::test]
    async fn pool_creates_connections() {
        let listener = TokioTcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let stats = Arc::new(StatisticsEngine::new());

        // Accept connections in background using async listener
        let accept_handle = tokio::spawn(async move {
            // Accept enough for initial pool + some extra
            for _ in 0..5 {
                if tokio::time::timeout(Duration::from_secs(2), listener.accept())
                    .await
                    .is_err()
                {
                    break;
                }
            }
        });

        let pool = AsyncConnectionPool::new(addr, Duration::from_secs(1), 2, stats)
            .await
            .unwrap();

        pool.ensure_initialized(Duration::from_secs(5))
            .await
            .unwrap();

        let conn = pool.get_connection().await;
        assert!(conn.is_ok());

        drop(pool);
        accept_handle.abort();
    }

    #[tokio::test]
    async fn pool_fails_on_invalid_address() {
        let stats = Arc::new(StatisticsEngine::new());

        // Use a port that's unlikely to be listening
        let addr: SocketAddr = "127.0.0.1:1".parse().unwrap();

        let pool = AsyncConnectionPool::new(addr, Duration::from_millis(100), 1, stats)
            .await
            .unwrap();

        // Should timeout waiting for initialization
        let result = pool.ensure_initialized(Duration::from_millis(500)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn get_connection_returns_valid_stream() {
        let listener = TokioTcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let stats = Arc::new(StatisticsEngine::new());

        let accept_handle = tokio::spawn(async move {
            // Accept and echo back
            if let Ok((mut stream, _)) = listener.accept().await {
                let mut buf = [0u8; 5];
                let _ = tokio::io::AsyncReadExt::read_exact(&mut stream, &mut buf).await;
                let _ = stream.write_all(&buf).await;
            }
        });

        let pool = AsyncConnectionPool::new(addr, Duration::from_secs(1), 1, stats)
            .await
            .unwrap();

        pool.ensure_initialized(Duration::from_secs(5))
            .await
            .unwrap();

        let mut conn = pool.get_connection().await.unwrap();

        // Verify we can write and read from the connection
        conn.write_all(b"hello").await.unwrap();
        let mut buf = [0u8; 5];
        tokio::io::AsyncReadExt::read_exact(&mut conn, &mut buf)
            .await
            .unwrap();
        assert_eq!(&buf, b"hello");

        let _ = accept_handle.await;
    }

    #[tokio::test]
    async fn pool_replenishes_after_use() {
        let listener = TokioTcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let stats = Arc::new(StatisticsEngine::new());

        // Accept multiple connections concurrently
        let accept_handle = tokio::spawn(async move {
            for _ in 0..5 {
                // Use timeout to avoid blocking forever
                let _ = tokio::time::timeout(Duration::from_secs(2), listener.accept()).await;
            }
        });

        // Use pool_size of 2 so replenishment is more visible
        let pool = AsyncConnectionPool::new(addr, Duration::from_millis(500), 2, stats)
            .await
            .unwrap();

        pool.ensure_initialized(Duration::from_secs(5))
            .await
            .unwrap();

        // Get first connection
        let conn1 = pool.get_connection().await;
        assert!(conn1.is_ok());
        drop(conn1);

        // Get second connection (was pre-created)
        let conn2 = pool.get_connection().await;
        assert!(conn2.is_ok());

        drop(pool);
        accept_handle.abort();
    }

    #[tokio::test]
    async fn drop_cancels_background_task() {
        let listener = TokioTcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let stats = Arc::new(StatisticsEngine::new());

        // Accept a few connections
        let accept_handle = tokio::spawn(async move {
            for _ in 0..3 {
                let _ = listener.accept().await;
            }
        });

        let pool = AsyncConnectionPool::new(addr, Duration::from_secs(1), 1, stats)
            .await
            .unwrap();

        pool.ensure_initialized(Duration::from_secs(5))
            .await
            .unwrap();

        // Drop the pool - this should cancel the background task
        drop(pool);

        // Give time for cancellation
        tokio::time::sleep(Duration::from_millis(100)).await;

        // The accept_handle may or may not complete depending on timing
        // but the important thing is no panic/crash
        accept_handle.abort();
    }

    #[tokio::test]
    async fn statistics_reference_returned() {
        let listener = TokioTcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let stats = Arc::new(StatisticsEngine::new());
        let stats_clone = stats.clone();

        let accept_handle = tokio::spawn(async move {
            let _ = listener.accept().await;
        });

        let pool = AsyncConnectionPool::new(addr, Duration::from_secs(1), 1, stats)
            .await
            .unwrap();

        // Verify statistics() returns the same Arc
        assert!(Arc::ptr_eq(pool.statistics(), &stats_clone));

        drop(pool);
        let _ = accept_handle.await;
    }

    #[tokio::test]
    async fn background_task_increments_error_on_connection_failure() {
        let listener = TokioTcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let stats = Arc::new(StatisticsEngine::new());

        // Accept initial connection then drop listener
        let accept_handle = tokio::spawn(async move {
            let _ = listener.accept().await;
            // Listener dropped here - subsequent connections will fail
        });

        let pool =
            AsyncConnectionPool::new(addr, Duration::from_millis(50), 1, stats.clone()).await.unwrap();

        pool.ensure_initialized(Duration::from_secs(5))
            .await
            .unwrap();

        // Consume the initial connection
        let _conn = pool.get_connection().await.unwrap();

        // Wait for accept_handle to finish (listener dropped)
        let _ = accept_handle.await;

        // Wait for background task to try creating connections and fail
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Error count should have increased
        let snapshot = stats.snapshot();
        assert!(
            snapshot.error_count >= 1,
            "expected error_count >= 1, got {}",
            snapshot.error_count
        );
    }

    #[tokio::test]
    async fn init_fails_on_connection_timeout() {
        // Use an address that will timeout (non-routable IP)
        let addr: SocketAddr = "10.255.255.1:18083".parse().unwrap();
        let stats = Arc::new(StatisticsEngine::new());

        let pool =
            AsyncConnectionPool::new(addr, Duration::from_millis(100), 1, stats).await.unwrap();

        // Should fail due to timeout during initialization
        let result = pool.ensure_initialized(Duration::from_millis(500)).await;
        assert!(result.is_err());

        match result {
            Err(BridgeError::Initialization(msg)) => {
                // Either timeout or connection failure message
                assert!(
                    msg.contains("timeout") || msg.contains("Connection") || msg.contains("did not initialize"),
                    "unexpected error message: {}", msg
                );
            }
            other => panic!("expected Initialization error, got {:?}", other),
        }
    }
}
