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
mod tests;
