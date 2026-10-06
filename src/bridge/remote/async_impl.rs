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
            connection: Mutex::new(Connection {
                reader: BufReader::new(read_half),
                writer: BufWriter::new(write_half),
                response_buffer: Vec::with_capacity(4096),
                in_flight: false,
            }),
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
///
/// # Cancellation
///
/// Calls are serialized over one connection. If a call is cancelled (e.g. by
/// `tokio::time::timeout`) mid-exchange, the connection is out of sync and later
/// calls return [BridgeError::Connection]; create a new bridge to recover.
pub struct AsyncRemoteBridge {
    connection: Mutex<Connection>,
}

/// Connection state, locked for a full request/response round trip.
struct Connection {
    reader: BufReader<tokio::net::tcp::OwnedReadHalf>,
    writer: BufWriter<tokio::net::tcp::OwnedWriteHalf>,
    response_buffer: Vec<u8>,
    /// Set while a round trip is in progress. Still set at the start of a call
    /// means a previous call was cancelled or failed mid-exchange, leaving the
    /// stream out of sync.
    in_flight: bool,
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

        let mut guard = self.connection.lock().await;
        let conn = &mut *guard;
        if conn.in_flight {
            return Err(BridgeError::Connection(std::io::Error::other(
                "connection out of sync after interrupted request; reconnect",
            )));
        }

        conn.in_flight = true;
        conn.writer.write_all(&frame).await?;
        conn.writer.flush().await?;

        let mut header = [0u8; FRAME_HEADER_LEN];
        conn.reader.read_exact(&mut header).await?;

        // Read the response data into reusable buffer
        conn.response_buffer.clear();
        conn.response_buffer.resize(frame_len(header), 0);
        conn.reader.read_exact(&mut conn.response_buffer).await?;
        conn.in_flight = false;

        decode_frame(&conn.response_buffer)
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

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_calls_receive_own_responses() {
        // Echo channel 0 back in the state so each caller can check its reply
        let proxy = MockProxy::handle(|request| {
            let mut state = SimulatorState::default();
            state.current_physics_time_s = request.payload.as_ref().unwrap().channels[0];
            Response::success_with(state)
        });
        let bridge = std::sync::Arc::new(AsyncRemoteBridge::new(&proxy.addr).await.unwrap());

        let tasks: Vec<_> = (0..200)
            .map(|i| {
                let bridge = bridge.clone();
                tokio::spawn(async move {
                    let mut control = ControlInputs::default();
                    control.channels[0] = i as f32;
                    let state = bridge.exchange_data(&control).await.unwrap();
                    assert_eq!(state.current_physics_time_s, i as f32, "task {i}");
                })
            })
            .collect();

        for task in tasks {
            task.await.unwrap();
        }
    }

    #[tokio::test]
    async fn cancelled_call_does_not_leak_response_to_next_call() {
        // First reply is slow so the caller times out after the request is sent
        let mut first = true;
        let proxy = MockProxy::handle(move |request| {
            if std::mem::take(&mut first) {
                std::thread::sleep(Duration::from_millis(200));
            }
            let mut state = SimulatorState::default();
            state.current_physics_time_s = request.payload.as_ref().unwrap().channels[0];
            Response::success_with(state)
        });
        let bridge = AsyncRemoteBridge::new(&proxy.addr).await.unwrap();

        let mut stale = ControlInputs::default();
        stale.channels[0] = 1.0;
        let cancelled = timeout(Duration::from_millis(50), bridge.exchange_data(&stale)).await;
        assert!(cancelled.is_err(), "first call should time out");

        let mut fresh = ControlInputs::default();
        fresh.channels[0] = 2.0;
        match bridge.exchange_data(&fresh).await {
            Err(BridgeError::Connection(_)) => {}
            Ok(state) => panic!("received stale response: {}", state.current_physics_time_s),
            Err(other) => panic!("expected Connection error, got {:?}", other),
        }
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
    async fn unit_ops_fail_on_proxy_error() {
        let proxy = MockProxy::respond(Response::error());
        let bridge = AsyncRemoteBridge::new(&proxy.addr).await.unwrap();

        assert!(bridge.enable_rc().await.is_err());
        assert!(bridge.disable_rc().await.is_err());
        assert!(bridge.reset_aircraft().await.is_err());
    }

    #[tokio::test]
    async fn server_disconnect_returns_error() {
        let proxy = MockProxy::hang_up();
        let bridge = AsyncRemoteBridge::new(&proxy.addr).await.unwrap();
        proxy.requests();

        assert!(bridge.enable_rc().await.is_err());
    }
}
