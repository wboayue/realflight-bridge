//! TCP proxy server for forwarding requests to the RealFlight simulator.
//!
//! The proxy server listens for client connections and forwards requests to the
//! local RealFlight simulator.
//!
//! ## Key Components
//!
//! - **[`ProxyServer`]**: Server struct that listens for client connections and processes requests.
//!
//! ## Usage
//!
//! ### Server Example
//! ```no_run
//! use realflight_bridge::{ProxyServer, BridgeError};
//!
//! fn main() -> Result<(), BridgeError> {
//!     let mut server = ProxyServer::new("0.0.0.0:8080")?;
//!     server.run()?; // Runs indefinitely until an error occurs
//!     Ok(())
//! }
//! ```
//!
//! ## Design Notes
//!
//! - **Synchronous Operation**: The server processes one client at a time, blocking until the client disconnects.

use std::io::{BufReader, BufWriter, Read, Write};
use std::net::{TcpListener, TcpStream};

use log::{error, info};
use postcard::{from_bytes, to_stdvec};

use crate::{BridgeError, SimulatorState};

use super::RealFlightBridge;
use super::local::RealFlightLocalBridge;
use super::remote::{Request, RequestType, Response};

#[cfg(test)]
mod tests;

/// Server struct for handling incoming client connections.
///
/// ### Examples
///
/// ```no_run
/// use realflight_bridge::{ProxyServer, BridgeError};
///
/// fn main() -> Result<(), BridgeError> {
///     let mut server = ProxyServer::new("0.0.0.0:8080")?;
///     server.run()?; // Runs indefinitely until an error occurs
///     Ok(())
/// }
/// ```
pub struct ProxyServer {
    listener: Option<TcpListener>, // TCP listener for incoming connections
    bridge: Option<Box<dyn RealFlightBridge + Send>>, // Bridge to simulator (None for stubbed mode)
}

impl ProxyServer {
    /// Creates a new server instance with a default local bridge.
    ///
    /// # Arguments
    /// * `bind_address` - The address to bind to (e.g., "0.0.0.0:8080").
    ///
    /// # Returns
    /// A `Result` containing the server instance or an error if binding or bridge creation fails.
    pub fn new(bind_address: &str) -> Result<Self, BridgeError> {
        let listener = TcpListener::bind(bind_address)?;
        let bridge = RealFlightLocalBridge::new()?;
        Ok(ProxyServer {
            listener: Some(listener),
            bridge: Some(Box::new(bridge)),
        })
    }

    /// Creates a new server instance with a custom bridge implementation.
    ///
    /// # Arguments
    /// * `bind_address` - The address to bind to (e.g., "0.0.0.0:8080").
    /// * `bridge` - The bridge implementation to use for simulator communication.
    ///
    /// # Returns
    /// A `Result` containing the server instance or an error if binding fails.
    pub fn with_bridge(
        bind_address: &str,
        bridge: Box<dyn RealFlightBridge + Send>,
    ) -> Result<Self, BridgeError> {
        let listener = TcpListener::bind(bind_address)?;
        Ok(ProxyServer {
            listener: Some(listener),
            bridge: Some(bridge),
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
                bridge: None,
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
    pub fn run(&mut self) -> Result<(), BridgeError> {
        let listener = self
            .listener
            .take()
            .ok_or_else(|| BridgeError::Initialization("Listener not initialized".into()))?;
        let stubbed = self.bridge.is_none();

        println!("Server listening on {}", listener.local_addr()?);

        // Accept incoming connections and handle them
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    handle_client(stream, self.bridge.as_deref())?;
                    if stubbed {
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
/// * `bridge` - Optional bridge for simulator communication (None for stubbed mode).
///
/// # Returns
/// A `Result` indicating success or an error.
fn handle_client(
    stream: TcpStream,
    bridge: Option<&(dyn RealFlightBridge + Send)>,
) -> Result<(), BridgeError> {
    let stubbed = bridge.is_none();
    if stubbed {
        info!("Running in stubbed mode");
    }

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
        let response = match bridge {
            Some(bridge) => process_request(request, bridge),
            None => {
                let response = process_request_stubbed(request);
                send_response(&mut writer, response)?;
                break; // Exit after one request in stubbed mode
            }
        };
        send_response(&mut writer, response)?;
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
) -> Result<(), BridgeError> {
    let response_bytes = to_stdvec(&response)
        .map_err(|e| BridgeError::SoapFault(format!("Failed to serialize response: {}", e)))?;
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
fn process_request(request: Request, bridge: &dyn RealFlightBridge) -> Response {
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
