//! TCP stub server for tests needing a real socket.
//!
//! This module provides a mock TCP server that simulates RealFlight's SOAP interface.
//! It loads canned responses from `testdata/responses/` and returns them based on
//! response keys passed during construction.
//!
//! # Usage
//!
//! ```ignore
//! let server = Server::new(&["reset-aircraft-200"]);
//! // Server is listening on 127.0.0.1:{server.port()} and will return the response
//! // from testdata/responses/reset-aircraft-200.xml
//! ```
//!
//! # Response Key Format
//!
//! Response keys follow the pattern `{action}-{status_code}`:
//! - `reset-aircraft-200` - Successful reset aircraft response
//! - `inject-uav-controller-interface-500` - Failed disable RC response
//! - `return-data-200` - Successful exchange data response

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread;

use super::SoapResponse;

/// A mock TCP server for testing SOAP client interactions.
pub struct Server {
    port: u16,
    handle: Option<thread::JoinHandle<()>>,
    running: Arc<AtomicBool>,
    requests: Arc<Mutex<Vec<String>>>,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        // wake the worker if it is blocked in accept
        let _ = TcpStream::connect(("127.0.0.1", self.port));

        if let Some(handle) = self.handle.take()
            && let Err(e) = handle.join()
        {
            eprintln!("error shutting down server: {:?}", e);
        }
    }
}

impl Server {
    /// Binds to a free port on 127.0.0.1 and serves `responses` in order,
    /// one per connection.
    pub fn new(responses: &[&str]) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let running = Arc::new(AtomicBool::new(true));
        let requests = Arc::new(Mutex::new(Vec::new()));

        let handle = spawn_worker(
            listener,
            responses.iter().map(|key| key.to_string()).collect(),
            Arc::clone(&running),
            Arc::clone(&requests),
        );

        Server {
            port,
            handle: Some(handle),
            running,
            requests,
        }
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    #[cfg_attr(not(feature = "rt-tokio"), allow(dead_code))]
    pub fn requests(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }
}

fn spawn_worker(
    listener: TcpListener,
    mut responses: VecDeque<String>,
    running: Arc<AtomicBool>,
    requests: Arc<Mutex<Vec<String>>>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        for incoming in listener.incoming() {
            if !running.load(Ordering::Relaxed) || responses.is_empty() {
                break;
            }

            let stream = match incoming {
                Ok(stream) => stream,
                Err(e) => {
                    eprintln!("connection error: {}", e);
                    break;
                }
            };

            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut status_line = String::new();
            if let Err(e) = reader.read_line(&mut status_line) {
                eprintln!("error reading status line: {}", e);
                break;
            }

            let request_body = read_request_body(&mut reader);
            if request_body.is_empty() {
                eprintln!("empty request, stopping");
                break;
            }

            requests.lock().unwrap().push(request_body);

            if let Some(response_key) = responses.pop_front() {
                send_response(&stream, &response_key);
            }
        }
    })
}

fn read_request_body(reader: &mut BufReader<TcpStream>) -> String {
    let content_length = content_length(reader);
    if content_length == 0 {
        return String::new();
    }

    let mut request_body = vec![0; content_length];
    reader.read_exact(&mut request_body).unwrap();

    String::from_utf8_lossy(&request_body).to_string()
}

fn content_length(reader: &mut BufReader<TcpStream>) -> usize {
    let mut content_length: Option<usize> = None;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).is_err() {
            return 0;
        }

        if line == "\r\n" || line.is_empty() {
            break;
        }

        if line.to_lowercase().starts_with("content-length:")
            && let Some(length) = line.split_whitespace().nth(1)
        {
            content_length = length.trim().parse().ok();
        }
    }
    content_length.unwrap_or(0)
}

/// Loads `testdata/responses/{key}.xml`; the status code is the key's last segment.
pub(crate) fn canned_response(key: &str) -> SoapResponse {
    let path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "testdata",
        "responses",
        &format!("{key}.xml"),
    ]
    .iter()
    .collect();

    SoapResponse {
        status_code: key.rsplit('-').next().unwrap().parse().unwrap(),
        body: std::fs::read_to_string(path).unwrap(),
    }
}

fn send_response(mut stream: &TcpStream, response_key: &str) {
    let SoapResponse { status_code, body } = canned_response(response_key);

    let mut buffer = String::new();
    buffer.push_str(&format!("HTTP/1.1 {} OK\r\n", status_code));
    buffer.push_str("Server: gSOAP/2.7\r\n");
    buffer.push_str("Content-Type: text/xml; charset=utf-8\r\n");
    buffer.push_str(&format!("Content-Length: {}\r\n", body.len()));
    buffer.push_str("Connection: close\r\n");
    buffer.push_str("\r\n");
    buffer.push_str(&body);

    stream.write_all(buffer.as_bytes()).unwrap();
    stream.flush().unwrap();
}
