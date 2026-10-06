//! This module provides a TCP based client for interacting with the RealFlight simulator on a remote machine.
//!
//! ## Key Components
//!
//! - **[`RequestType`]**: Enumerates the types of requests that can be sent (e.g., [RequestType::EnableRC], [RequestType::ExchangeData]).
//! - **[`Request`]**: Defines the structure of client requests, including an optional [ControlInputs] payload.
//! - **[`Response`]**: Defines server responses: a [`ResponseStatus`] and optional [SimulatorState] payload.
//! - **[`RemoteError`]**: Proxy-side error carried by [`ResponseStatus::Error`]; the client rebuilds it as a [BridgeError].
//! - **[`RealFlightRemoteBridge`]**: Client struct for connecting to the server and sending requests.
//!
//! ## Usage
//!
//! ### Client Example
//! ```no_run
//! use realflight_bridge::{RealFlightBridge, RealFlightRemoteBridge, BridgeError, ControlInputs};
//!
//! fn main() -> Result<(), BridgeError> {
//!     let client = RealFlightRemoteBridge::new("127.0.0.1:18083")?;
//!     client.disable_rc()?; // Allow control via RealFlight link
//!     let control = ControlInputs::default(); // Initialize control inputs
//!     let state = client.exchange_data(&control)?; // Exchange data
//!     Ok(())
//! }
//! ```

#[cfg(feature = "rt-tokio")]
mod async_impl;
#[cfg(feature = "rt-tokio")]
pub use async_impl::{AsyncRemoteBridge, AsyncRemoteBridgeBuilder};

use std::cell::RefCell;
use std::io::{BufReader, BufWriter};
use std::time::Duration;
use std::{
    io::Write,
    net::{SocketAddr, TcpStream, ToSocketAddrs},
};

use crate::defaults;
use crate::{BridgeError, ControlInputs, SimulatorState};

use super::RealFlightBridge;
use super::wire::RequestRef;
use super::wire::frame::{decode_frame, encode_frame};
use super::wire::frame_io::read_frame;
pub use super::wire::{RemoteError, Request, RequestType, Response, ResponseStatus};

#[cfg(test)]
mod tests;

/// Resolves `address` to its first socket address.
pub(crate) fn resolve(address: &str) -> std::io::Result<SocketAddr> {
    address
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid address"))
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
        self.call(RequestType::EnableRC, None)?.into_unit()
    }

    /// Disables remote control on the simulator. (Enables control by the RealFlight link.)
    fn disable_rc(&self) -> Result<(), BridgeError> {
        self.call(RequestType::DisableRC, None)?.into_unit()
    }

    /// Resets the aircraft state in the simulator.
    fn reset_aircraft(&self) -> Result<(), BridgeError> {
        self.call(RequestType::ResetAircraft, None)?.into_unit()
    }

    /// Sends [ControlInputs] to the simulator and receives the updated [SimulatorState].
    ///
    /// # Arguments
    /// * `control` - The [ControlInputs] to send.
    ///
    /// # Returns
    /// The [SimulatorState] or an error if no state is returned.
    fn exchange_data(&self, control: &ControlInputs) -> Result<SimulatorState, BridgeError> {
        self.call(RequestType::ExchangeData, Some(control))?
            .into_state()
    }
}

impl RealFlightRemoteBridge {
    /// Creates a new client instance connected to the specified address.
    ///
    /// # Arguments
    /// * `address` - The server address (e.g., "127.0.0.1:18083").
    ///
    /// # Returns
    /// A `Result` containing the new client instance or an I/O error.
    pub fn new(address: &str) -> std::io::Result<Self> {
        Self::with_timeout(address, defaults::REMOTE_TIMEOUT)
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
        let stream = TcpStream::connect_timeout(&resolve(address)?, timeout)?;
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
    /// A `Result` containing the server's response or an error.
    fn call(
        &self,
        request_type: RequestType,
        payload: Option<&ControlInputs>,
    ) -> Result<Response, BridgeError> {
        let frame = encode_frame(&RequestRef {
            request_type,
            payload,
        })?;

        let mut writer = self.writer.borrow_mut();
        writer.write_all(&frame)?;
        writer.flush()?;

        let mut response_buffer = self.response_buffer.borrow_mut();
        read_frame(&mut *self.reader.borrow_mut(), &mut response_buffer)?;
        decode_frame(&response_buffer)
    }
}
