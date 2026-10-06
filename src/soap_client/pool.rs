//! Connection pool for TCP connections to the RealFlight simulator.
//!
//! The RealFlight SoapServer requires a new connection for each request.
//! This pool pre-creates connections in the background to hide latency.

use std::{
    net::{SocketAddr, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use crossbeam_channel::{Receiver, Sender, bounded};
use log::{debug, error};

use crate::BridgeError;
use crate::StatisticsEngine;

/// Pre-creates TCP connections in a background thread to hide connection latency.
///
/// The RealFlight SoapServer requires a new connection for each request.
/// The idea for the pool is to create the next connection
/// in the background while the current request is being processed.
pub(crate) struct ConnectionPool {
    next_socket: Receiver<TcpStream>,
    creator_thread: Option<thread::JoinHandle<()>>,
    running: Arc<AtomicBool>,
    initialized: Arc<AtomicBool>,
    init_error: Arc<Mutex<Option<String>>>,
    statistics: Arc<StatisticsEngine>,
}

impl ConnectionPool {
    pub fn new(
        addr: SocketAddr,
        connect_timeout: Duration,
        pool_size: usize,
        statistics: Arc<StatisticsEngine>,
    ) -> Result<Self, BridgeError> {
        let (sender, receiver) = bounded(pool_size);

        let mut pool = ConnectionPool {
            next_socket: receiver,
            creator_thread: None,
            running: Arc::new(AtomicBool::new(true)),
            initialized: Arc::new(AtomicBool::new(false)),
            init_error: Arc::new(Mutex::new(None)),
            statistics,
        };

        pool.initialize_pool(sender, addr, connect_timeout, pool_size)?;

        Ok(pool)
    }

    pub(crate) fn ensure_initialized(&self, init_timeout: Duration) -> Result<(), BridgeError> {
        let now = Instant::now();
        while !self.initialized.load(Ordering::Relaxed) {
            // Check for initialization error
            if let Some(err) = self
                .init_error
                .lock()
                .ok()
                .and_then(|g| g.as_ref().cloned())
            {
                return Err(BridgeError::Initialization(format!(
                    "Connection pool initialization failed: {}",
                    err
                )));
            }
            if now.elapsed() > init_timeout {
                return Err(BridgeError::Initialization(format!(
                    "Connection pool did not initialize. Waited for {:?}.",
                    init_timeout
                )));
            }
            thread::sleep(Duration::from_millis(100));
        }
        Ok(())
    }

    // Start the background thread that creates new connections
    fn initialize_pool(
        &mut self,
        sender: Sender<TcpStream>,
        simulator_address: SocketAddr,
        connect_timeout: Duration,
        pool_size: usize,
    ) -> Result<(), BridgeError> {
        let running = Arc::clone(&self.running);
        let initialized = Arc::clone(&self.initialized);
        let init_error = Arc::clone(&self.init_error);
        let statistics = Arc::clone(&self.statistics);

        let worker = thread::Builder::new().name("connection-pool".to_string());
        let handle = worker.spawn(move || {
            debug!("Creating {} connections in pool.", pool_size);

            // Create initial connections
            for i in 0..pool_size {
                match TcpStream::connect_timeout(&simulator_address, connect_timeout) {
                    Ok(stream) => {
                        if let Err(e) = sender.send(stream) {
                            let msg = format!("Failed to queue initial connection {}: {}", i, e);
                            error!("{}", msg);
                            if let Ok(mut guard) = init_error.lock() {
                                *guard = Some(msg);
                            }
                            return;
                        }
                    }
                    Err(e) => {
                        let msg = format!(
                            "Failed to connect to simulator at {}: {}",
                            simulator_address, e
                        );
                        error!("{}", msg);
                        if let Ok(mut guard) = init_error.lock() {
                            *guard = Some(msg);
                        }
                        return;
                    }
                }
            }

            initialized.store(true, Ordering::Relaxed);

            // Continue creating connections as needed
            while running.load(Ordering::Relaxed) {
                if sender.is_full() {
                    thread::sleep(connect_timeout / 2);
                    continue;
                }

                match TcpStream::connect_timeout(&simulator_address, connect_timeout) {
                    Ok(stream) => {
                        if let Err(e) = sender.send(stream) {
                            error!("Error sending connection: {}", e);
                            statistics.increment_error_count();
                        }
                    }
                    Err(e) => {
                        error!("Error creating connection: {}", e);
                        statistics.increment_error_count();
                        thread::sleep(connect_timeout);
                    }
                }
            }
        });

        self.creator_thread = Some(handle.map_err(|e| {
            BridgeError::Initialization(format!("Failed to spawn connection pool thread: {}", e))
        })?);
        Ok(())
    }

    // Get a new connection, consuming it
    pub fn get_connection(&self) -> Result<TcpStream, BridgeError> {
        self.next_socket.recv().map_err(|e| {
            BridgeError::Initialization(format!("Failed to get connection from pool: {}", e))
        })
    }
}

impl Drop for ConnectionPool {
    fn drop(&mut self) {
        // Signal the creator thread to stop
        self.running.store(false, Ordering::Relaxed);

        // Wait for the creator thread to finish
        if let Some(handle) = self.creator_thread.take() {
            let _ = handle.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    const INIT_TIMEOUT: Duration = Duration::from_secs(5);

    fn get_available_port() -> u16 {
        TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port()
    }

    fn local_addr(port: u16) -> SocketAddr {
        SocketAddr::from(([127, 0, 0, 1], port))
    }

    fn test_pool(
        port: u16,
        connect_timeout: Duration,
        pool_size: usize,
        stats: Arc<StatisticsEngine>,
    ) -> Result<ConnectionPool, BridgeError> {
        ConnectionPool::new(local_addr(port), connect_timeout, pool_size, stats)
    }

    mod pool_creation {
        use super::*;

        #[test]
        fn succeeds_with_listening_server() {
            let port = get_available_port();
            let _listener = TcpListener::bind(format!("127.0.0.1:{}", port)).unwrap();

            let stats = Arc::new(StatisticsEngine::new());

            let pool = test_pool(port, Duration::from_millis(100), 2, stats);
            assert!(pool.is_ok());
        }

        #[test]
        fn fails_when_server_unreachable() {
            let port = get_available_port();
            // Don't start a server - connection should fail

            let stats = Arc::new(StatisticsEngine::new());

            let pool = test_pool(port, Duration::from_millis(100), 2, stats);
            assert!(pool.is_ok());

            let pool = pool.unwrap();
            let result = pool.ensure_initialized(INIT_TIMEOUT);
            assert!(result.is_err());

            match result {
                Err(BridgeError::Initialization(msg)) => {
                    assert!(msg.contains("Failed to connect"));
                }
                other => panic!("expected Initialization error, got {:?}", other),
            }
        }
    }

    mod ensure_initialized {
        use super::*;

        #[test]
        fn returns_ok_when_initialized() {
            let port = get_available_port();
            let _listener = TcpListener::bind(format!("127.0.0.1:{}", port)).unwrap();

            let stats = Arc::new(StatisticsEngine::new());

            let pool = test_pool(port, Duration::from_millis(100), 2, stats).unwrap();
            let result = pool.ensure_initialized(INIT_TIMEOUT);
            assert!(result.is_ok());
        }

        #[test]
        fn multiple_calls_succeed_after_init() {
            let port = get_available_port();
            let _listener = TcpListener::bind(format!("127.0.0.1:{}", port)).unwrap();

            let stats = Arc::new(StatisticsEngine::new());

            let pool = test_pool(port, Duration::from_millis(100), 2, stats).unwrap();

            // First call waits for initialization
            assert!(pool.ensure_initialized(INIT_TIMEOUT).is_ok());
            // Subsequent calls return immediately
            assert!(pool.ensure_initialized(INIT_TIMEOUT).is_ok());
            assert!(pool.ensure_initialized(INIT_TIMEOUT).is_ok());
        }
    }

    mod get_connection {
        use super::*;
        use std::io::Write;

        #[test]
        fn returns_valid_connection() {
            let port = get_available_port();
            let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).unwrap();

            let stats = Arc::new(StatisticsEngine::new());

            let pool = test_pool(port, Duration::from_millis(100), 2, stats).unwrap();
            pool.ensure_initialized(INIT_TIMEOUT).unwrap();

            // Accept the connections the pool created
            let _conn1 = listener.accept().unwrap();
            let _conn2 = listener.accept().unwrap();

            let conn = pool.get_connection();
            assert!(conn.is_ok());

            // Verify the connection is usable
            let mut stream = conn.unwrap();
            let result = stream.write_all(b"test");
            assert!(result.is_ok());
        }

        #[test]
        fn connections_are_consumed() {
            let port = get_available_port();
            let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).unwrap();

            // pool_size = 2, so we can get exactly 2 connections initially
            let stats = Arc::new(StatisticsEngine::new());

            let pool = test_pool(port, Duration::from_millis(100), 2, stats).unwrap();
            pool.ensure_initialized(INIT_TIMEOUT).unwrap();

            // Accept the connections the pool created
            let _conn1 = listener.accept().unwrap();
            let _conn2 = listener.accept().unwrap();

            // Get both connections from pool
            let conn1 = pool.get_connection();
            assert!(conn1.is_ok());

            let conn2 = pool.get_connection();
            assert!(conn2.is_ok());
        }
    }

    mod pool_drop {
        use super::*;

        #[test]
        fn stops_creator_thread() {
            let port = get_available_port();
            let _listener = TcpListener::bind(format!("127.0.0.1:{}", port)).unwrap();

            let stats = Arc::new(StatisticsEngine::new());

            let pool = test_pool(port, Duration::from_millis(100), 2, stats).unwrap();
            pool.ensure_initialized(INIT_TIMEOUT).unwrap();

            // Drop should complete without hanging
            drop(pool);
        }

        #[test]
        fn drop_is_safe_before_initialization() {
            let port = get_available_port();
            // No listener - pool will fail to initialize

            let stats = Arc::new(StatisticsEngine::new());

            let pool = test_pool(port, Duration::from_millis(100), 2, stats).unwrap();
            // Don't wait for initialization, just drop
            drop(pool);
        }
    }

    mod background_connection_creation {
        use super::*;

        #[test]
        fn creates_new_connections_after_consumption() {
            let port = get_available_port();
            let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).unwrap();
            listener.set_nonblocking(true).unwrap();

            let stats = Arc::new(StatisticsEngine::new());

            let pool = test_pool(port, Duration::from_millis(100), 1, stats).unwrap();
            pool.ensure_initialized(INIT_TIMEOUT).unwrap();

            // Accept initial connection
            thread::sleep(Duration::from_millis(50));
            let mut accepted = 0;
            while let Ok(_) = listener.accept() {
                accepted += 1;
            }
            assert!(accepted >= 1, "should have accepted at least 1 connection");

            // Get a connection (consumes it)
            let _conn = pool.get_connection().unwrap();

            // Wait for background thread to create a new one
            thread::sleep(Duration::from_millis(200));

            // Should have created at least one more connection
            let mut more_accepted = 0;
            while let Ok(_) = listener.accept() {
                more_accepted += 1;
            }
            assert!(
                more_accepted >= 1,
                "background should have created more connections"
            );
        }
    }

    mod error_statistics {
        use super::*;

        #[test]
        fn increments_error_on_connection_failure() {
            let port = get_available_port();
            let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).unwrap();

            let stats = Arc::new(StatisticsEngine::new());

            let pool = test_pool(port, Duration::from_millis(50), 1, stats.clone()).unwrap();
            pool.ensure_initialized(INIT_TIMEOUT).unwrap();

            // Accept initial connection
            let _conn = listener.accept().unwrap();

            // Drop the listener so subsequent connections fail
            drop(listener);

            // Consume the connection to trigger background creation
            let _conn = pool.get_connection().unwrap();

            // Wait for background thread to try creating a connection and fail
            thread::sleep(Duration::from_millis(200));

            // Error count should have increased
            let snapshot = stats.snapshot();
            assert!(
                snapshot.error_count >= 1,
                "expected error_count >= 1, got {}",
                snapshot.error_count
            );
        }
    }
}
