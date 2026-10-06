//! Async implementation of the remote bridge for RealFlight simulator.

use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::time::timeout;

use crate::bridge::AsyncBridge;
use crate::defaults;
use crate::{BridgeError, ControlInputs, SimulatorState};

use super::frame::{FRAME_HEADER_LEN, decode_frame, encode_frame, frame_len};
use super::{RequestRef, RequestType, Response, resolve};

/// Builder for AsyncRemoteBridge.
///
/// Configure options synchronously, then call `build()` to connect.
#[derive(Debug, Clone)]
pub struct AsyncRemoteBridgeBuilder {
    address: String,
    connect_timeout: Duration,
}

impl AsyncRemoteBridgeBuilder {
    /// Creates a new builder with the specified address.
    pub fn new(address: &str) -> Self {
        Self {
            address: address.to_string(),
            connect_timeout: defaults::REMOTE_TIMEOUT,
        }
    }

    /// Sets the connection timeout.
    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// Builds the AsyncRemoteBridge, connecting to the server.
    pub async fn build(self) -> Result<AsyncRemoteBridge, BridgeError> {
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
            reader: Mutex::new(BufReader::new(read_half)),
            writer: Mutex::new(BufWriter::new(write_half)),
            response_buffer: Mutex::new(Vec::with_capacity(4096)),
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
///     // Or with custom timeout
///     let bridge = AsyncRemoteBridge::builder("192.168.1.100:18083")
///         .timeout(Duration::from_secs(10))
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
pub struct AsyncRemoteBridge {
    reader: Mutex<BufReader<tokio::net::tcp::OwnedReadHalf>>,
    writer: Mutex<BufWriter<tokio::net::tcp::OwnedWriteHalf>>,
    response_buffer: Mutex<Vec<u8>>,
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
        let frame = encode_frame(&RequestRef {
            request_type,
            payload,
        })?;

        let mut writer = self.writer.lock().await;
        writer.write_all(&frame).await?;
        writer.flush().await?;

        drop(writer); // Release lock before reading

        let mut reader = self.reader.lock().await;
        let mut header = [0u8; FRAME_HEADER_LEN];
        reader.read_exact(&mut header).await?;

        // Read the response data into reusable buffer
        let mut response_buffer = self.response_buffer.lock().await;
        response_buffer.clear();
        response_buffer.resize(frame_len(header), 0);
        reader.read_exact(&mut response_buffer).await?;

        decode_frame(&response_buffer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::AsyncBridge;
    use crate::bridge::remote::test_support::MockProxy;
    use std::net::TcpListener;

    // ========================================================================
    // Connection Tests
    // ========================================================================

    #[tokio::test]
    async fn connects_to_server() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();

        // Accept one connection in background
        let handle = std::thread::spawn(move || {
            let _ = listener.accept();
        });

        let result = AsyncRemoteBridge::builder(&addr)
            .timeout(Duration::from_secs(1))
            .build()
            .await;

        assert!(result.is_ok());
        let _ = handle.join();
    }

    #[tokio::test]
    async fn builder_sets_timeout() {
        let builder =
            AsyncRemoteBridgeBuilder::new("127.0.0.1:12345").timeout(Duration::from_millis(100));

        assert_eq!(builder.connect_timeout, Duration::from_millis(100));
    }

    #[tokio::test]
    async fn connection_timeout_returns_error() {
        // Use a non-routable address to trigger timeout
        let result = AsyncRemoteBridge::builder("10.255.255.1:12345")
            .timeout(Duration::from_millis(100))
            .build()
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn invalid_address_returns_error() {
        let result = AsyncRemoteBridge::new("not-a-valid-address").await;
        assert!(result.is_err());
    }

    // ========================================================================
    // Operation Tests
    // ========================================================================

    #[tokio::test]
    async fn unit_ops_send_matching_requests() {
        let proxy = MockProxy::respond(Response::success());
        let bridge = AsyncRemoteBridge::new(&proxy.addr).await.unwrap();

        bridge.enable_rc().await.unwrap();
        bridge.disable_rc().await.unwrap();
        bridge.reset_aircraft().await.unwrap();
        drop(bridge);

        let types: Vec<_> = proxy
            .requests()
            .into_iter()
            .map(|r| r.request_type)
            .collect();
        assert_eq!(
            types,
            [
                RequestType::EnableRC,
                RequestType::DisableRC,
                RequestType::ResetAircraft
            ]
        );
    }

    #[tokio::test]
    async fn exchange_data_sends_inputs_and_returns_state() {
        let proxy = MockProxy::respond(Response::success_with(SimulatorState::default()));
        let bridge = AsyncRemoteBridge::new(&proxy.addr).await.unwrap();
        let mut control = ControlInputs::default();
        control.channels[2] = 1.0;

        let state = bridge.exchange_data(&control).await.unwrap();
        assert_eq!(state, SimulatorState::default());
        drop(bridge);

        let requests = proxy.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].request_type, RequestType::ExchangeData);
        assert_eq!(requests[0].payload, Some(control));
    }

    // ========================================================================
    // Error Handling Tests
    // ========================================================================

    #[tokio::test]
    async fn malformed_response_is_invalid_data() {
        let proxy = MockProxy::reply_raw(vec![0xFF, 0xFF, 0xFF, 0xFF]);
        let bridge = AsyncRemoteBridge::new(&proxy.addr).await.unwrap();

        match bridge.enable_rc().await {
            Err(BridgeError::Connection(e)) => {
                assert_eq!(e.kind(), std::io::ErrorKind::InvalidData);
            }
            other => panic!("expected Connection(InvalidData), got {:?}", other),
        }
    }

    #[tokio::test]
    async fn server_disconnect_returns_error() {
        let proxy = MockProxy::hang_up();
        let bridge = AsyncRemoteBridge::new(&proxy.addr).await.unwrap();
        proxy.requests();

        assert!(bridge.enable_rc().await.is_err());
    }
}
