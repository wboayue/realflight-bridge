//! This module provides a TCP based proxy for interacting with the RealFlight simulator on a remote machine.
//! The system includes a client ([RealFlightRemoteBridge]) for sending requests and a proxy server ([ProxyServer]) for
//! handling them.
//!
//! ## Key Components
//!
//! - **[`RequestType`]**: Enumerates the types of requests that can be sent (e.g., [RequestType::EnableRC], [RequestType::ExchangeData]).
//! - **[`Request`]**: Defines the structure of client requests, including an optional [ControlInputs] payload.
//! - **[`Response`]**: Defines server responses, including a status and optional [SimulatorState] payload.
//! - **[`RealFlightRemoteBridge`]**: Client struct for connecting to the server and sending requests.
//! - **[`ProxyServer`]**: Server struct that listens for client connections and processes requests.
//!
//! ## Usage
//!
//! ### Client Example
//! ```no_run
//! use std::error::Error;
//! use realflight_bridge::{RealFlightBridge, RealFlightRemoteBridge, ControlInputs};
//!
//! fn main() -> Result<(), Box<dyn Error>> {
//!     let mut client = RealFlightRemoteBridge::new("127.0.0.1:18083")?;
//!     client.disable_rc()?; // Allow control via RealFlight link
//!     let control = ControlInputs::default(); // Initialize control inputs
//!     let state = client.exchange_data(&control)?; // Exchange data
//!     Ok(())
//! }
//! ```
//!
//! ### Server Example
//! ```no_run
//! use std::error::Error;
//! use realflight_bridge::ProxyServer;
//!
//! fn main() -> Result<(), Box<dyn Error>> {
//!     let mut server = ProxyServer::new("0.0.0.0:8080")?; // Normal mode
//!     server.run()?; // Runs indefinitely until an error occurs
//!     Ok(())
//! }
//! ```
//!
//! ## Design Notes
//!
//! - **Synchronous Operation**: The server processes one client at a time, blocking until the client disconnects.

use std::cell::RefCell;
use std::io::{BufReader, BufWriter};
use std::time::Duration;
use std::{
    error::Error,
    io::{Read, Write},
    net::{TcpListener, TcpStream, ToSocketAddrs},
};

use log::{error, info};
use postcard::{from_bytes, to_stdvec};
use serde::{Deserialize, Serialize};

use crate::{ControlInputs, SimulatorState};

use super::RealFlightBridge;
use super::local::RealFlightLocalBridge;

#[cfg(test)]
mod tests;

/// Defines the types of requests that can be sent to the server.
#[derive(Debug, Serialize, Deserialize)]
pub enum RequestType {
    /// Enable remote control
    EnableRC,
    /// Disable remote control (enable control by ReaFlight link)
    DisableRC,
    /// Reset the aircraft state (like pressing space-bar in the simulator)
    ResetAircraft,
    /// Send [ControlInputs] and receive [SimulatorState]
    ExchangeData,
}

/// Represents a request sent from the client to the server.
#[derive(Debug, Serialize, Deserialize)]
pub struct Request {
    /// Type of request being made
    pub request_type: RequestType,
    /// Optional [ControlInputs] data
    pub payload: Option<ControlInputs>,
}

/// Represents a response sent from the server to the client.
#[derive(Debug, Serialize, Deserialize)]
pub struct Response {
    /// Indicates success or failure
    pub status: ResponseStatus,
    /// Optional [SimulatorState] data
    pub payload: Option<SimulatorState>,
}

/// Indicates the status of a response.
#[derive(Debug, Serialize, Deserialize)]
pub enum ResponseStatus {
    /// Operation completed successfully
    Success,
    /// Operation failed
    Error,
}

impl Response {
    fn success() -> Self {
        Self {
            status: ResponseStatus::Success,
            payload: None,
        }
    }

    fn success_with(state: SimulatorState) -> Self {
        Self {
            status: ResponseStatus::Success,
            payload: Some(state),
        }
    }

    fn error() -> Self {
        Self {
            status: ResponseStatus::Error,
            payload: None,
        }
    }

    /// Convert a Result into a Response, logging errors with context
    fn from_result<E: std::fmt::Display>(result: Result<(), E>, context: &str) -> Self {
        match result {
            Ok(()) => Self::success(),
            Err(e) => {
                error!("Error {}: {}", context, e);
                Self::error()
            }
        }
    }
}

/// Client struct for managing TCP communication with the simulator server.
pub struct RealFlightRemoteBridge {
    reader: RefCell<BufReader<TcpStream>>, // Buffered reader for incoming data
    writer: RefCell<BufWriter<TcpStream>>, // Buffered writer for outgoing data
}

impl RealFlightBridge for RealFlightRemoteBridge {
    /// Enables remote control on the simulator.
    fn enable_rc(&self) -> Result<(), Box<dyn Error>> {
        self.send_request(RequestType::EnableRC, None)?;
        Ok(())
    }

    /// Disables remote control on the simulator. (Enables control by the RealFlight link.)
    fn disable_rc(&self) -> Result<(), Box<dyn Error>> {
        self.send_request(RequestType::DisableRC, None)?;
        Ok(())
    }

    /// Resets the aircraft state in the simulator.
    fn reset_aircraft(&self) -> Result<(), Box<dyn Error>> {
        self.send_request(RequestType::ResetAircraft, None)?;
        Ok(())
    }

    /// Sends [ControlInputs] to the simulator and receives the updated [SimulatorState].
    ///
    /// # Arguments
    /// * `control` - The [ControlInputs] to send.
    ///
    /// # Returns
    /// The [SimulatorState] or an error if no state is returned.
    fn exchange_data(&self, control: &ControlInputs) -> Result<SimulatorState, Box<dyn Error>> {
        let response = self.send_request(RequestType::ExchangeData, Some(control.clone()))?;
        if let Some(state) = response.payload {
            Ok(state)
        } else {
            error!("No payload in response: {:?}", response.status);
            Err("No payload in response".into())
        }
    }
}

impl RealFlightRemoteBridge {
    /// Default connection timeout (5 seconds)
    const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

    /// Creates a new client instance connected to the specified address.
    ///
    /// # Arguments
    /// * `address` - The server address (e.g., "127.0.0.1:18083").
    ///
    /// # Returns
    /// A `Result` containing the new client instance or an I/O error.
    pub fn new(address: &str) -> std::io::Result<Self> {
        Self::with_timeout(address, Self::DEFAULT_TIMEOUT)
    }

    /// Creates a new client instance with a custom timeout.
    ///
    /// # Arguments
    /// * `address` - The server address (e.g., "127.0.0.1:18083").
    /// * `timeout` - Connection timeout duration.
    ///
    /// # Returns
    /// A `Result` containing the new client instance or an I/O error.
    pub fn with_timeout(address: &str, timeout: Duration) -> std::io::Result<Self> {
        let addr = address.to_socket_addrs()?.next().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid address")
        })?;

        let stream = TcpStream::connect_timeout(&addr, timeout)?;
        stream.set_nodelay(true)?;

        Ok(RealFlightRemoteBridge {
            reader: RefCell::new(BufReader::new(stream.try_clone()?)),
            writer: RefCell::new(BufWriter::new(stream)),
        })
    }

    /// Sends a request to the server and receives a response.
    ///
    /// # Arguments
    /// * `request_type` - The type of request to send.
    /// * `payload` - Optional [ControlInputs] to include in the request.
    ///
    /// # Returns
    /// A `Result` containing the server's response or an I/O error.
    fn send_request(
        &self,
        request_type: RequestType,
        payload: Option<ControlInputs>,
    ) -> std::io::Result<Response> {
        let request = Request {
            request_type,
            payload,
        };

        // Serialize the request to a byte vector
        let request_bytes = to_stdvec(&request)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        let mut writer = self.writer.borrow_mut();

        // Send the length of the request (4 bytes)
        let length_bytes = (request_bytes.len() as u32).to_be_bytes();
        writer.write_all(&length_bytes)?;

        // Send the serialized request data
        writer.write_all(&request_bytes)?;
        writer.flush()?;

        let mut reader = self.reader.borrow_mut();

        // Read the response length (4 bytes)
        let mut length_buffer = [0u8; 4];
        reader.read_exact(&mut length_buffer)?;
        let response_length = u32::from_be_bytes(length_buffer) as usize;

        // Read the response data
        let mut response_buffer = vec![0u8; response_length];
        reader.read_exact(&mut response_buffer)?;

        // Deserialize the response
        let response: Response = from_bytes(&response_buffer)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        Ok(response)
    }
}

/// Server struct for handling incoming client connections.
///
/// ### Examples
///
/// ```no_run
/// use std::error::Error;
/// use realflight_bridge::ProxyServer;
///
/// fn main() -> Result<(), Box<dyn Error>> {
///     let mut server = ProxyServer::new("0.0.0.0:8080")?;
///     server.run()?; // Runs indefinitely until an error occurs
///     Ok(())
/// }
/// ```
pub struct ProxyServer {
    listener: Option<TcpListener>, // TCP listener for incoming connections
    stubbed: bool,                 // Whether to run in stubbed mode (no real simulator)
}

impl ProxyServer {
    /// Creates a new server instance.
    ///
    /// # Arguments
    /// * `bind_address` - The address to bind to (e.g., "0.0.0.0:8080").
    ///
    /// # Returns
    /// A `Result` containing the server instance or an I/O error if binding fails.
    pub fn new(bind_address: &str) -> std::io::Result<Self> {
        let listener = TcpListener::bind(bind_address)?;
        Ok(ProxyServer {
            listener: Some(listener),
            stubbed: false,
        })
    }

    /// Creates a new server instance in stubbed mode.
    /// This mode is used for testing purposes and does not require a real simulator.
    ///
    /// # Returns
    /// A `Result` containing a tuple of (server instance, bound address) or an I/O error.
    #[cfg(test)]
    pub fn new_stubbed() -> std::io::Result<(Self, String)> {
        let bind_address = "127.0.0.1:0";
        let listener = TcpListener::bind(bind_address)?;
        let local_addr = listener.local_addr()?.to_string();
        Ok((
            ProxyServer {
                listener: Some(listener),
                stubbed: true,
            },
            local_addr,
        ))
    }

    /// Runs the server, listening for incoming connections.
    /// Server should run indefinitely until an error occurs.
    /// Server is designed to handle one client at a time.
    ///
    /// # Returns
    /// A `Result` indicating success or an error.
    pub fn run(&mut self) -> Result<(), Box<dyn Error>> {
        let listener = self.listener.take().ok_or("Listener not initialized")?;

        println!("Server listening on {}", listener.local_addr()?);

        // Accept incoming connections and handle them
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    handle_client(stream, self.stubbed)?; // Handle each client
                    if self.stubbed {
                        break; // Exit after handling one client in stubbed mode
                    }
                }
                Err(e) => {
                    eprintln!("Failed to accept connection: {}", e);
                }
            }
        }

        Ok(())
    }
}

/// Handles a single client connection.
///
/// # Arguments
/// * `stream` - The TCP stream for the client.
/// * `stubbed` - Whether to run in stubbed mode.
///
/// # Returns
/// A `Result` indicating success or an error.
fn handle_client(stream: TcpStream, stubbed: bool) -> Result<(), Box<dyn Error>> {
    // Initialize bridge if not in stubbed mode
    let bridge = if stubbed {
        info!("Running in stubbed mode");
        None
    } else {
        Some(RealFlightLocalBridge::new()?)
    };

    info!("New client connected: {}", stream.peer_addr()?);

    stream.set_nodelay(true)?; // Disable Nagle's algorithm

    let mut reader = BufReader::new(&stream);
    let mut writer = BufWriter::new(&stream);

    let mut length_buffer = [0u8; 4]; // Buffer for message length

    // Process requests until client disconnects

    while reader.read_exact(&mut length_buffer).is_ok() {
        let msg_length = u32::from_be_bytes(length_buffer) as usize;

        // Read the request data
        let mut buffer = vec![0u8; msg_length];
        reader.read_exact(&mut buffer)?;

        // Deserialize the request
        let request: Request = match from_bytes(&buffer) {
            Ok(req) => req,
            Err(e) => {
                error!("Failed to deserialize request: {}", e);
                continue;
            }
        };

        // Process request based on mode
        if stubbed {
            let response = process_request_stubbed(request);
            send_response(&mut writer, response)?;
            break;
        } else if let Some(bridge) = &bridge {
            let response = process_request(request, bridge);
            send_response(&mut writer, response)?;
        };
    }

    info!("Client disconnected: {}", stream.peer_addr()?);
    Ok(())
}

/// Sends a response to the client.
///
/// # Arguments
/// * `writer` - The buffered writer for the TCP stream.
/// * `response` - The response to send.
///
/// # Returns
/// A `Result` indicating success or an error.
fn send_response(
    writer: &mut BufWriter<&TcpStream>,
    response: Response,
) -> Result<(), Box<dyn Error>> {
    let response_bytes = to_stdvec(&response)?;
    let length_bytes = (response_bytes.len() as u32).to_be_bytes();

    writer.write_all(&length_bytes)?;
    writer.write_all(&response_bytes)?;
    writer.flush()?;

    Ok(())
}

/// Processes a request using forwarding to simulator via [RealFlightBridge].
///
/// # Arguments
/// * `request` - The client's request.
/// * `bridge`  - The [RealFlightBridge] instance.
///
/// # Returns
/// The response to send back to the client.
fn process_request(request: Request, bridge: &RealFlightLocalBridge) -> Response {
    match request.request_type {
        RequestType::EnableRC => Response::from_result(bridge.enable_rc(), "enabling RC"),
        RequestType::DisableRC => Response::from_result(bridge.disable_rc(), "disabling RC"),
        RequestType::ResetAircraft => {
            Response::from_result(bridge.reset_aircraft(), "resetting aircraft")
        }
        RequestType::ExchangeData => match request.payload {
            Some(payload) => match bridge.exchange_data(&payload) {
                Ok(state) => Response::success_with(state),
                Err(e) => {
                    error!("Error exchanging data: {}", e);
                    Response::error()
                }
            },
            None => Response::error(),
        },
    }
}

/// Processes a request in stubbed mode (no real simulator).
///
/// # Arguments
/// * `request` - The client's request.
///
/// # Returns
/// A mocked response for testing purposes.
fn process_request_stubbed(request: Request) -> Response {
    match request.request_type {
        RequestType::EnableRC | RequestType::DisableRC | RequestType::ResetAircraft => {
            Response::success()
        }
        RequestType::ExchangeData => Response::success_with(SimulatorState::default()),
    }
}
