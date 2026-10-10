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

use std::cell::{Cell, RefCell};
use std::io::{BufReader, BufWriter, ErrorKind};
use std::time::Duration;
use std::{
    io::Write,
    net::{SocketAddr, TcpStream, ToSocketAddrs},
};

use crate::defaults;
use crate::{BridgeError, ControlInputs, SimulatorState};

use super::RealFlightBridge;
use super::wire::RequestRef;
use super::wire::frame::{decode_frame, encode_frame_into};
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
///
/// Each read and write is bounded by an I/O timeout (default 100 ms), so a
/// stalled proxy returns an error rather than blocking the caller.
///
/// If a call fails mid-exchange (e.g. I/O error, timeout or invalid response frame), the
/// connection is out of sync and later calls return [BridgeError::Connection];
/// create a new bridge to recover.
pub struct RealFlightRemoteBridge {
    reader: RefCell<BufReader<TcpStream>>, // Buffered reader for incoming data
    writer: RefCell<BufWriter<TcpStream>>, // Buffered writer for outgoing data
    request_buffer: RefCell<Vec<u8>>,      // Reusable buffer for requests
    response_buffer: RefCell<Vec<u8>>,     // Reusable buffer for responses
    in_flight: Cell<bool>,                 // Set while a round trip is in progress
    io_timeout: Duration,                  // Read/write timeout, for error messages
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
    /// Uses a 5 s connect timeout and a 100 ms I/O timeout; see
    /// [`with_timeouts`](Self::with_timeouts).
    ///
    /// # Arguments
    /// * `address` - The server address (e.g., "127.0.0.1:18083").
    ///
    /// # Returns
    /// A `Result` containing the new client instance or an I/O error.
    pub fn new(address: &str) -> std::io::Result<Self> {
        Self::with_timeouts(
            address,
            defaults::REMOTE_TIMEOUT,
            defaults::REMOTE_IO_TIMEOUT,
        )
    }

    /// Creates a new client instance with a custom connect timeout.
    ///
    /// `timeout` bounds only the connect; the I/O timeout stays at the 100 ms
    /// default. Use [`with_timeouts`](Self::with_timeouts) to set both.
    ///
    /// # Arguments
    /// * `address` - The server address (e.g., "127.0.0.1:18083").
    /// * `timeout` - Connection timeout duration.
    ///
    /// # Returns
    /// A `Result` containing the new client instance or an I/O error.
    pub fn with_timeout(address: &str, timeout: Duration) -> std::io::Result<Self> {
        Self::with_timeouts(address, timeout, defaults::REMOTE_IO_TIMEOUT)
    }

    /// Creates a new client instance with custom connect and I/O timeouts.
    ///
    /// `io` bounds each read and write of a call. A call that times out returns
    /// [BridgeError::Connection] with kind [`TimedOut`](std::io::ErrorKind::TimedOut)
    /// and leaves the connection out of sync; create a new bridge to recover.
    /// Keep `io` above the proxy's request timeout so a stalled simulator comes
    /// back as an error reply instead.
    ///
    /// # Arguments
    /// * `address` - The server address (e.g., "127.0.0.1:18083").
    /// * `connect` - Connection timeout duration.
    /// * `io` - Read/write timeout for each call. Must be non-zero.
    ///
    /// # Returns
    /// A `Result` containing the new client instance or an I/O error.
    pub fn with_timeouts(address: &str, connect: Duration, io: Duration) -> std::io::Result<Self> {
        let stream = TcpStream::connect_timeout(&resolve(address)?, connect)?;
        stream.set_nodelay(true)?;
        stream.set_read_timeout(Some(io))?;
        stream.set_write_timeout(Some(io))?;

        Ok(RealFlightRemoteBridge {
            reader: RefCell::new(BufReader::new(stream.try_clone()?)),
            writer: RefCell::new(BufWriter::new(stream)),
            request_buffer: RefCell::new(Vec::with_capacity(256)),
            response_buffer: RefCell::new(Vec::with_capacity(4096)),
            in_flight: Cell::new(false),
            io_timeout: io,
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
        if self.in_flight.get() {
            return Err(BridgeError::Connection(std::io::Error::other(
                "connection out of sync after failed request; reconnect",
            )));
        }

        let mut request_buffer = self.request_buffer.borrow_mut();
        encode_frame_into(
            &RequestRef {
                request_type,
                payload,
            },
            &mut request_buffer,
        )?;

        self.in_flight.set(true);
        let mut response_buffer = self.response_buffer.borrow_mut();
        self.round_trip(&request_buffer, &mut response_buffer)
            .map_err(|e| self.timeout_error(e))?;
        self.in_flight.set(false);
        decode_frame(&response_buffer)
    }

    /// Writes `request` and reads the response frame into `response`.
    fn round_trip(&self, request: &[u8], response: &mut Vec<u8>) -> std::io::Result<()> {
        let mut writer = self.writer.borrow_mut();
        writer.write_all(request)?;
        writer.flush()?;
        read_frame(&mut *self.reader.borrow_mut(), response)
    }

    /// Reports a socket timeout as `TimedOut`; platforms differ (`WouldBlock`
    /// on Unix).
    fn timeout_error(&self, e: std::io::Error) -> std::io::Error {
        match e.kind() {
            ErrorKind::WouldBlock | ErrorKind::TimedOut => std::io::Error::new(
                ErrorKind::TimedOut,
                format!("no response from proxy within {:?}", self.io_timeout),
            ),
            _ => e,
        }
    }
}
