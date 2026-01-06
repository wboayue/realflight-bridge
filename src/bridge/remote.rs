//! This module provides a TCP based client for interacting with the RealFlight simulator on a remote machine.
//!
//! ## Key Components
//!
//! - **[`RequestType`]**: Enumerates the types of requests that can be sent (e.g., [RequestType::EnableRC], [RequestType::ExchangeData]).
//! - **[`Request`]**: Defines the structure of client requests, including an optional [ControlInputs] payload.
//! - **[`Response`]**: Defines server responses, including a status and optional [SimulatorState] payload.
//! - **[`RealFlightRemoteBridge`]**: Client struct for connecting to the server and sending requests.
//!
//! ## Usage
//!
//! ### Client Example
//! ```no_run
//! use realflight_bridge::{RealFlightBridge, RealFlightRemoteBridge, BridgeError, ControlInputs};
//!
//! fn main() -> Result<(), BridgeError> {
//!     let mut client = RealFlightRemoteBridge::new("127.0.0.1:18083")?;
//!     client.disable_rc()?; // Allow control via RealFlight link
//!     let control = ControlInputs::default(); // Initialize control inputs
//!     let state = client.exchange_data(&control)?; // Exchange data
//!     Ok(())
//! }
//! ```

use std::cell::RefCell;
use std::io::{BufReader, BufWriter};
use std::time::Duration;
use std::{
    io::{Read, Write},
    net::{TcpStream, ToSocketAddrs},
};

use log::error;
use postcard::{from_bytes, to_stdvec};
use serde::{Deserialize, Serialize};

use crate::{BridgeError, ControlInputs, SimulatorState};

use super::RealFlightBridge;

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

/// Client struct for managing TCP communication with the simulator server.
pub struct RealFlightRemoteBridge {
    reader: RefCell<BufReader<TcpStream>>, // Buffered reader for incoming data
    writer: RefCell<BufWriter<TcpStream>>, // Buffered writer for outgoing data
    response_buffer: RefCell<Vec<u8>>,     // Reusable buffer for responses
}

impl RealFlightBridge for RealFlightRemoteBridge {
    /// Enables remote control on the simulator.
    fn enable_rc(&self) -> Result<(), BridgeError> {
        self.send_request(RequestType::EnableRC, None)?;
        Ok(())
    }

    /// Disables remote control on the simulator. (Enables control by the RealFlight link.)
    fn disable_rc(&self) -> Result<(), BridgeError> {
        self.send_request(RequestType::DisableRC, None)?;
        Ok(())
    }

    /// Resets the aircraft state in the simulator.
    fn reset_aircraft(&self) -> Result<(), BridgeError> {
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
    fn exchange_data(&self, control: &ControlInputs) -> Result<SimulatorState, BridgeError> {
        let response = self.send_request(RequestType::ExchangeData, Some(control.clone()))?;
        if let Some(state) = response.payload {
            Ok(state)
        } else {
            error!("No payload in response: {:?}", response.status);
            Err(BridgeError::SoapFault("No payload in response".to_string()))
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
            response_buffer: RefCell::new(Vec::with_capacity(4096)),
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

        // Read the response data into reusable buffer
        let mut response_buffer = self.response_buffer.borrow_mut();
        response_buffer.clear();
        response_buffer.resize(response_length, 0);
        reader.read_exact(&mut response_buffer)?;

        // Deserialize the response
        let response: Response = from_bytes(&response_buffer)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        Ok(response)
    }
}
