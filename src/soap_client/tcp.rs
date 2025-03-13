//! Provides and implementation of a SOAP client that uses the TCP protocol.

use std::{
    error::Error,
    io::{BufRead, BufReader, Read, Write},
    net::TcpStream,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
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
        self.send_request(&mut stream, action, &envelope);
        self.statistics.increment_request_count();

        match self.read_response(&mut BufReader::new(stream)) {
            Some(response) => Ok(response),
            None => Err("Failed to read response".into()),
        }
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
    fn send_request(&self, stream: &mut TcpStream, action: &str, envelope: &str) {
        let mut request = String::with_capacity(HEADER_LEN + envelope.len() + action.len());

        request.push_str("POST / HTTP/1.1\r\n");
        request.push_str(&format!("Soapaction: '{}'\r\n", action));
        request.push_str(&format!("Content-Length: {}\r\n", envelope.len()));
        request.push_str("Content-Type: text/xml;charset=utf-8\r\n");
        request.push_str("\r\n");
        request.push_str(envelope);

        stream
            .write_all(request.as_bytes())
            .expect("Failed to write request");
        stream.flush().expect("Failed to flush request");
    }

    /// Reads the raw response from the simulator.
    fn read_response(&self, stream: &mut BufReader<TcpStream>) -> Option<SoapResponse> {
        let mut status_line = String::new();

        if let Err(e) = stream.read_line(&mut status_line) {
            eprintln!("Error reading status line: {}", e);
            return None;
        }

        if status_line.is_empty() {
            return None;
        }

        let status_code: u32 = status_line
            .split_whitespace()
            .nth(1)
            .unwrap()
            .parse()
            .unwrap();

        // Read headers
        let mut headers = String::new();
        let mut content_length: Option<usize> = None;
        loop {
            let mut line = String::new();
            stream.read_line(&mut line).unwrap();
            if line == "\r\n" {
                break; // End of headers
            }
            if line.to_lowercase().starts_with("content-length:") {
                if let Some(length) = line.split_whitespace().nth(1) {
                    content_length = length.trim().parse().ok();
                }
            }
            headers.push_str(&line);
        }

        // Read the body based on Content-Length
        if let Some(length) = content_length {
            let mut body = vec![0; length];
            stream.read_exact(&mut body).unwrap();
            let body = String::from_utf8_lossy(&body).to_string();

            Some(SoapResponse { status_code, body })
        } else {
            None
        }
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
            statistics,
        };

        pool.initialize_pool(sender)?;

        Ok(pool)
    }

    pub(crate) fn ensure_pool_initialized(&self) -> Result<()> {
        let now = Instant::now();
        while !self.initialized.load(Ordering::Relaxed) {
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
        let statistics = Arc::clone(&self.statistics);

        let worker = thread::Builder::new().name("connection-pool".to_string());
        let handle = worker.spawn(move || {
            debug!("Creating {} connection in pool.", config.pool_size);
            let simulator_address = config
                .simulator_host
                .parse()
                .expect("Invalid simulator host");

            for _ in 0..config.pool_size {
                let stream = TcpStream::connect(&simulator_address).unwrap();
                sender.send(stream).unwrap();
            }

            initialized.store(true, Ordering::Relaxed);

            while running.load(Ordering::Relaxed) {
                if sender.is_full() {
                    thread::sleep(config.connect_timeout / 2);
                    continue;
                }

                match TcpStream::connect_timeout(&simulator_address, config.connect_timeout) {
                    Ok(stream) => match sender.send(stream) {
                        Ok(_) => {}
                        Err(e) => {
                            error!("Error sending connection: {}", e);
                            statistics.increment_error_count();
                        }
                    },
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
