//! Provides and implementation of a SOAP client that uses the TCP protocol.

use std::{
    error::Error,
    io::{BufRead, BufReader, Read, Write},
    net::TcpStream,
    sync::{Arc, Mutex},
    thread,
};

use crossbeam_channel::{bounded, Receiver, Sender};
use log::error;

use crate::{encode_envelope, Configuration, SoapClient, SoapResponse, StatisticsEngine};

const HEADER_LEN: usize = 120;

pub(crate) struct TcpSoapClient {
    pub(crate) statistics: Arc<StatisticsEngine>,
    pub(crate) connection_manager: ConnectionPool,
}

impl SoapClient for TcpSoapClient {
    fn send_action(&self, action: &str, body: &str) -> Result<SoapResponse, Box<dyn Error>> {
        eprintln!("Sending action: {}", action);

        let envelope = encode_envelope(action, body);
        let mut stream = self.connection_manager.get_connection()?;
        self.send_request(&mut stream, action, &envelope);
        self.statistics.increment_request_count();

        match self.read_response(&mut BufReader::new(stream)) {
            Some(response) => Ok(response),
            None => Err("Failed to read response".into()),
        }
    }
}

impl TcpSoapClient {
    pub fn new(
        configuration: Configuration,
        statistics: Arc<StatisticsEngine>,
    ) -> Result<Self, Box<dyn Error>> {
        let connection_manager = ConnectionPool::new(configuration, statistics.clone())?;
        Ok(TcpSoapClient {
            statistics,
            connection_manager,
        })
    }

    fn send_request(&self, stream: &mut TcpStream, action: &str, envelope: &str) {
        let mut request = String::with_capacity(HEADER_LEN + envelope.len() + action.len());

        request.push_str("POST / HTTP/1.1\r\n");
        request.push_str(&format!("Soapaction: '{}'\r\n", action));
        request.push_str(&format!("Content-Length: {}\r\n", envelope.len()));
        request.push_str("Content-Type: text/xml;charset=utf-8\r\n");
        request.push_str("\r\n");
        request.push_str(envelope);

        stream.write_all(request.as_bytes()).unwrap();
    }

    fn read_response(&self, stream: &mut BufReader<TcpStream>) -> Option<SoapResponse> {
        let mut status_line = String::new();

        if let Err(e) = stream.read_line(&mut status_line) {
            eprintln!("Error reading status line: {}", e);
            return None;
        }

        if status_line.is_empty() {
            return None;
        }
        // eprintln!("Status Line: '{}'", status_line.trim());
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

        // println!("Headers:\n{}", headers);
        // println!("content length:\n{}", content_length.unwrap());

        // Read the body based on Content-Length
        if let Some(length) = content_length {
            // let mut body = String::with_capacity(length);
            // stream.read_to_string(&mut body).unwrap();

            let mut body = vec![0; length];
            stream.read_exact(&mut body).unwrap();
            let body = String::from_utf8_lossy(&body).to_string();
            // println!("Body: {}", r);

            Some(SoapResponse { status_code, body })
        } else {
            None
        }
    }
}

pub(crate) struct ConnectionPool {
    config: Configuration,
    next_socket: Receiver<TcpStream>,
    creator_thread: Option<thread::JoinHandle<()>>,
    running: Arc<Mutex<bool>>,
    statistics: Arc<StatisticsEngine>,
}

impl ConnectionPool {
    pub fn new(
        config: Configuration,
        statistics: Arc<StatisticsEngine>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let (sender, receiver) = bounded(config.pool_size);

        let running = Arc::new(Mutex::new(true));

        let mut manager = ConnectionPool {
            config,
            next_socket: receiver,
            creator_thread: None,
            running,
            statistics,
        };

        manager.start_socket_creator(sender)?;

        Ok(manager)
    }

    // Start the background thread that creates new connections
    fn start_socket_creator(
        &mut self,
        sender: Sender<TcpStream>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let config = self.config.clone();
        let running = Arc::clone(&self.running);
        let statistics = Arc::clone(&self.statistics);

        eprintln!("Creating {} connections...", config.pool_size);
        for _ in 0..config.pool_size {
            let stream = Self::create_connection(&config, &statistics)?;
            sender.send(stream).unwrap();
        }

        let handle = thread::spawn(move || {
            while *running.lock().unwrap() {
                if sender.is_full() && sender.capacity().unwrap() > 0 {
                    thread::sleep(config.retry_delay);
                    continue;
                }

                eprintln!("Creating new connection...");
                let connection = Self::create_connection(&config, &statistics).unwrap();
                sender.send(connection).unwrap();
            }
        });

        self.creator_thread = Some(handle);
        Ok(())
    }

    // Create a new TCP connection with timeout
    fn create_connection(
        config: &Configuration,
        statistics: &Arc<StatisticsEngine>,
    ) -> Result<TcpStream, Box<dyn std::error::Error>> {
        let addr = config.simulator_url.parse()?;
        for _ in 0..10 {
            match TcpStream::connect_timeout(&addr, config.connect_timeout) {
                Ok(stream) => {
                    return Ok(stream);
                }
                Err(e) => {
                    statistics.increment_error_count();
                    error!("Error creating connection: {}", e);
                }
            }
        }
        Err("Failed to create connection".into())
    }

    // Get a new connection, consuming it
    pub fn get_connection(&self) -> Result<TcpStream, Box<dyn std::error::Error>> {
        Ok(self.next_socket.recv()?)
    }
}

impl Drop for ConnectionPool {
    fn drop(&mut self) {
        // Signal the creator thread to stop
        if let Ok(mut running) = self.running.lock() {
            *running = false;
        }

        // Wait for the creator thread to finish
        if let Some(handle) = self.creator_thread.take() {
            let _ = handle.join();
        }
    }
}
