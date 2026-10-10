//! Async implementation of the remote bridge for RealFlight simulator.

use std::time::Duration;

use tokio::io::{AsyncWriteExt, BufReader, BufWriter};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::time::timeout;

use crate::bridge::AsyncBridge;
use crate::defaults;
use crate::timeout::{ensure_nonzero, timed_out};
use crate::{BridgeError, ControlInputs, SimulatorState};

use super::{RequestType, Response, resolve};
use crate::bridge::wire::RequestRef;
use crate::bridge::wire::frame::{decode_frame, encode_frame_into};
use crate::bridge::wire::frame_io::read_frame_async;

/// Builder for AsyncRemoteBridge.
///
/// Configure options synchronously, then call `build()` to connect.
#[derive(Debug, Clone)]
pub struct AsyncRemoteBridgeBuilder {
    address: String,
    connect_timeout: Duration,
    io_timeout: Duration,
}

impl AsyncRemoteBridgeBuilder {
    /// Creates a new builder with the specified address.
    pub fn new(address: &str) -> Self {
        Self {
            address: address.to_string(),
            connect_timeout: defaults::REMOTE_TIMEOUT,
            io_timeout: defaults::REMOTE_IO_TIMEOUT,
        }
    }

    /// Sets the connection timeout (default 5 s).
    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// Sets the timeout for each call's request/response round trip (default
    /// 100 ms). Must be non-zero; `Duration::MAX` disables it.
    ///
    /// A call that times out returns [BridgeError::Connection] with kind
    /// [`TimedOut`](std::io::ErrorKind::TimedOut) and leaves the connection out of
    /// sync; create a new bridge to recover. Keep it above the proxy's request
    /// timeout so a stalled simulator comes back as an error reply instead.
    #[must_use]
    pub fn io_timeout(mut self, timeout: Duration) -> Self {
        self.io_timeout = timeout;
        self
    }

    /// Builds the AsyncRemoteBridge, connecting to the server. Fails if the I/O
    /// timeout is zero.
    pub async fn build(self) -> Result<AsyncRemoteBridge, BridgeError> {
        ensure_nonzero("io_timeout", self.io_timeout)
            .map_err(|e| BridgeError::Initialization(e.to_string()))?;
        let addr = resolve(&self.address).map_err(|e| {
            BridgeError::Initialization(format!("Invalid address '{}': {}", self.address, e))
        })?;

        let stream = timeout(self.connect_timeout, TcpStream::connect(addr))
            .await
            .map_err(|_| {
                BridgeError::Initialization(format!(
                    "Connection timeout after {:?}",
                    self.connect_timeout
                ))
            })?
            .map_err(|e| BridgeError::Initialization(format!("Connection failed: {}", e)))?;

        stream
            .set_nodelay(true)
            .map_err(|e| BridgeError::Initialization(format!("Failed to set nodelay: {}", e)))?;

        let (read_half, write_half) = stream.into_split();

        Ok(AsyncRemoteBridge {
            connection: Mutex::new(Connection {
                reader: BufReader::new(read_half),
                writer: BufWriter::new(write_half),
                request_buffer: Vec::with_capacity(256),
                response_buffer: Vec::with_capacity(4096),
                in_flight: false,
            }),
            io_timeout: self.io_timeout,
        })
    }
}

/// Async client for interacting with a remote RealFlight simulator via a proxy server.
///
/// # Examples
///
/// ```no_run
/// use realflight_bridge::{AsyncBridge, AsyncRemoteBridge, ControlInputs};
/// use std::time::Duration;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // Connect to a remote proxy server
///     let bridge = AsyncRemoteBridge::new("192.168.1.100:18083").await?;
///
///     // Or with custom timeouts
///     let bridge = AsyncRemoteBridge::builder("192.168.1.100:18083")
///         .timeout(Duration::from_secs(10))
///         .io_timeout(Duration::from_millis(20))
///         .build()
///         .await?;
///
///     // Create sample control inputs
///     let inputs = ControlInputs::default();
///
///     // Exchange data with the simulator
///     let state = bridge.exchange_data(&inputs).await?;
///     println!("Current airspeed: {:?}", state.airspeed_mps);
///
///     Ok(())
/// }
/// ```
///
/// # Timeouts and cancellation
///
/// Calls are serialized over one connection, and each round trip is bounded by
/// the I/O timeout (default 100 ms, see [`AsyncRemoteBridgeBuilder::io_timeout`]).
/// If a call times out, fails or is cancelled mid-exchange, the connection is out
/// of sync and later calls return [BridgeError::Connection]; create a new bridge
/// to recover.
pub struct AsyncRemoteBridge {
    connection: Mutex<Connection>,
    io_timeout: Duration,
}

/// Connection state, locked for a full request/response round trip.
struct Connection {
    reader: BufReader<tokio::net::tcp::OwnedReadHalf>,
    writer: BufWriter<tokio::net::tcp::OwnedWriteHalf>,
    request_buffer: Vec<u8>,
    response_buffer: Vec<u8>,
    /// Set while a round trip is in progress. Still set at the start of a call
    /// means a previous call was cancelled or failed mid-exchange, leaving the
    /// stream out of sync.
    in_flight: bool,
}

impl Connection {
    /// Writes the encoded request and reads the response frame.
    async fn round_trip(&mut self) -> std::io::Result<()> {
        self.writer.write_all(&self.request_buffer).await?;
        self.writer.flush().await?;
        read_frame_async(&mut self.reader, &mut self.response_buffer).await
    }
}

impl AsyncBridge for AsyncRemoteBridge {
    async fn exchange_data(&self, control: &ControlInputs) -> Result<SimulatorState, BridgeError> {
        self.call(RequestType::ExchangeData, Some(control))
            .await?
            .into_state()
    }

    async fn enable_rc(&self) -> Result<(), BridgeError> {
        self.call(RequestType::EnableRC, None).await?.into_unit()
    }

    async fn disable_rc(&self) -> Result<(), BridgeError> {
        self.call(RequestType::DisableRC, None).await?.into_unit()
    }

    async fn reset_aircraft(&self) -> Result<(), BridgeError> {
        self.call(RequestType::ResetAircraft, None)
            .await?
            .into_unit()
    }
}

impl AsyncRemoteBridge {
    /// Creates a new AsyncRemoteBridge connected to the specified address.
    pub async fn new(address: &str) -> Result<Self, BridgeError> {
        AsyncRemoteBridgeBuilder::new(address).build().await
    }

    /// Returns a builder for custom configuration.
    pub fn builder(address: &str) -> AsyncRemoteBridgeBuilder {
        AsyncRemoteBridgeBuilder::new(address)
    }

    /// Sends a request to the server and receives a response.
    async fn call(
        &self,
        request_type: RequestType,
        payload: Option<&ControlInputs>,
    ) -> Result<Response, BridgeError> {
        let mut guard = self.connection.lock().await;
        let conn = &mut *guard;
        if conn.in_flight {
            return Err(BridgeError::Connection(std::io::Error::other(
                "connection out of sync after interrupted request; reconnect",
            )));
        }

        encode_frame_into(
            &RequestRef {
                request_type,
                payload,
            },
            &mut conn.request_buffer,
        )?;

        conn.in_flight = true;
        timeout(self.io_timeout, conn.round_trip())
            .await
            .map_err(|_| timed_out("proxy", self.io_timeout))??;
        conn.in_flight = false;

        decode_frame(&conn.response_buffer)
    }
}

#[cfg(test)]
mod tests;
