//! Provides and implementation of a SOAP client that uses the TCP protocol.

use std::{
    error::Error,
    io::{BufRead, BufReader, Read, Write},
    net::TcpStream,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

use anyhow::{anyhow, Result};
use crossbeam_channel::{bounded, Receiver, Sender};
use log::{debug, error};

use crate::bridge::local::{encode_envelope, Configuration, SoapClient};
use crate::{SoapResponse, StatisticsEngine};

/// Size of header for request body
const HEADER_LEN: usize = 120;
const INITIALIZATION_TIMEOUT: Duration = Duration::from_secs(5);

/// Implementation of a SOAP client for RealFlight Link that uses the TCP protocol.
pub(crate) struct TcpSoapClient {
    /// Statistics engine for tracking performance
    pub(crate) statistics: Arc<StatisticsEngine>,
    /// Connection pool for managing TCP connections
    pub(crate) connection_pool: ConnectionPool,
}

impl SoapClient for TcpSoapClient {
    /// Sends a SOAP action to the simulator and returns the response.
    ///
    /// # Arguments
    /// * `action` - The SOAP action to send.
    /// * `body`   - The body of the SOAP request.
    fn send_action(&self, action: &str, body: &str) -> Result<SoapResponse, Box<dyn Error>> {
        let envelope = encode_envelope(action, body);
        let mut stream = self.connection_pool.get_connection()?;
        self.send_request(&mut stream, action, &envelope)?;
        self.statistics.increment_request_count();

        self.read_response(&mut BufReader::new(stream))
    }
}

impl TcpSoapClient {
    /// Creates a new TCP SOAP client.
    pub fn new(
        configuration: Configuration,
        statistics: Arc<StatisticsEngine>,
    ) -> Result<Self, Box<dyn Error>> {
        let connection_pool = ConnectionPool::new(configuration, statistics.clone())?;
        Ok(TcpSoapClient {
            statistics,
            connection_pool,
        })
    }

    pub(crate) fn ensure_pool_initialized(&self) -> Result<()> {
        self.connection_pool.ensure_pool_initialized()?;
        Ok(())
    }

    /// Sends a request to the simulator.
    fn send_request(
        &self,
        stream: &mut TcpStream,
        action: &str,
        envelope: &str,
    ) -> Result<(), Box<dyn Error>> {
        let mut request = String::with_capacity(HEADER_LEN + envelope.len() + action.len());

        request.push_str("POST / HTTP/1.1\r\n");
        request.push_str(&format!("Soapaction: '{}'\r\n", action));
        request.push_str(&format!("Content-Length: {}\r\n", envelope.len()));
        request.push_str("Content-Type: text/xml;charset=utf-8\r\n");
        request.push_str("\r\n");
        request.push_str(envelope);

        stream.write_all(request.as_bytes())?;
        stream.flush()?;
        Ok(())
    }

    /// Reads the raw response from the simulator.
    fn read_response(
        &self,
        stream: &mut BufReader<TcpStream>,
    ) -> Result<SoapResponse, Box<dyn Error>> {
        let mut status_line = String::new();
        stream.read_line(&mut status_line)?;

        if status_line.is_empty() {
            return Err("Empty response from simulator".into());
        }

        let status_code: u32 = status_line
            .split_whitespace()
            .nth(1)
            .ok_or("Malformed HTTP status line: missing status code")?
            .parse()
            .map_err(|e| format!("Invalid HTTP status code: {}", e))?;

        // Read headers
        let mut content_length: Option<usize> = None;
        loop {
            let mut line = String::new();
            stream.read_line(&mut line)?;
            if line == "\r\n" {
                break; // End of headers
            }
            if line.to_lowercase().starts_with("content-length:")
                && let Some(length) = line.split_whitespace().nth(1)
            {
                content_length = length.trim().parse().ok();
            }
        }

        // Read the body based on Content-Length
        let length = content_length.ok_or("Missing Content-Length header")?;
        let mut body = vec![0; length];
        stream.read_exact(&mut body)?;
        let body = String::from_utf8_lossy(&body).to_string();

        Ok(SoapResponse { status_code, body })
    }
}

/// The RealFlight SoapServer requires a new connection for each request.
/// The idea for the pool is to create the next connection
/// in the background while the current request is being processed.
pub(crate) struct ConnectionPool {
    config: Configuration,
    next_socket: Receiver<TcpStream>,
    creator_thread: Option<thread::JoinHandle<()>>,
    running: Arc<AtomicBool>,
    initialized: Arc<AtomicBool>,
    init_error: Arc<Mutex<Option<String>>>,
    statistics: Arc<StatisticsEngine>,
}

impl ConnectionPool {
    pub fn new(
        config: Configuration,
        statistics: Arc<StatisticsEngine>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let (sender, receiver) = bounded(config.pool_size);

        let mut pool = ConnectionPool {
            config,
            next_socket: receiver,
            creator_thread: None,
            running: Arc::new(AtomicBool::new(true)),
            initialized: Arc::new(AtomicBool::new(false)),
            init_error: Arc::new(Mutex::new(None)),
            statistics,
        };

        pool.initialize_pool(sender)?;

        Ok(pool)
    }

    pub(crate) fn ensure_pool_initialized(&self) -> Result<()> {
        let now = Instant::now();
        while !self.initialized.load(Ordering::Relaxed) {
            // Check for initialization error
            if let Some(err) = self
                .init_error
                .lock()
                .ok()
                .and_then(|g| g.as_ref().cloned())
            {
                return Err(anyhow!("Connection pool initialization failed: {}", err));
            }
            if now.elapsed() > INITIALIZATION_TIMEOUT {
                return Err(anyhow!(
                    "Connection pool did not initialize. Waited for {:?}.",
                    INITIALIZATION_TIMEOUT
                ));
            }
            thread::sleep(Duration::from_millis(100));
        }
        Ok(())
    }

    // Start the background thread that creates new connections
    fn initialize_pool(&mut self, sender: Sender<TcpStream>) -> Result<()> {
        let config = self.config.clone();
        let running = Arc::clone(&self.running);
        let initialized = Arc::clone(&self.initialized);
        let init_error = Arc::clone(&self.init_error);
        let statistics = Arc::clone(&self.statistics);

        let worker = thread::Builder::new().name("connection-pool".to_string());
        let handle = worker.spawn(move || {
            debug!("Creating {} connections in pool.", config.pool_size);

            let simulator_address = match config.simulator_host.parse() {
                Ok(addr) => addr,
                Err(e) => {
                    let msg = format!("Invalid simulator host '{}': {}", config.simulator_host, e);
                    error!("{}", msg);
                    if let Ok(mut guard) = init_error.lock() {
                        *guard = Some(msg);
                    }
                    return;
                }
            };

            // Create initial connections
            for i in 0..config.pool_size {
                match TcpStream::connect(simulator_address) {
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
                            config.simulator_host, e
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
                    thread::sleep(config.connect_timeout / 2);
                    continue;
                }

                match TcpStream::connect_timeout(&simulator_address, config.connect_timeout) {
                    Ok(stream) => {
                        if let Err(e) = sender.send(stream) {
                            error!("Error sending connection: {}", e);
                            statistics.increment_error_count();
                        }
                    }
                    Err(e) => {
                        error!("Error creating connection: {}", e);
                        statistics.increment_error_count();
                        thread::sleep(config.connect_timeout);
                    }
                }
            }
        });

        self.creator_thread = Some(handle?);
        Ok(())
    }

    // Get a new connection, consuming it
    pub fn get_connection(&self) -> Result<TcpStream, Box<dyn std::error::Error>> {
        Ok(self.next_socket.recv()?)
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
