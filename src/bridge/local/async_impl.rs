//! Async implementation of the local bridge for RealFlight simulator.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use crate::bridge::AsyncBridge;
use crate::soap_client::AsyncSoapClient;
use crate::soap_client::tcp_async::AsyncTcpSoapClient;
use crate::{BridgeError, ControlInputs, SimulatorState, Statistics, StatisticsEngine};

use super::encode_control_inputs;

const EMPTY_BODY: &str = "";
const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_millis(5);
const DEFAULT_INIT_TIMEOUT: Duration = Duration::from_secs(5);
/// Pool pre-creates next connection to hide latency. Only one connection needed at a time.
const DEFAULT_POOL_SIZE: usize = 1;

/// Builder for AsyncLocalBridge.
///
/// Configure options synchronously, then call `build()` to connect.
#[derive(Debug, Clone)]
pub struct AsyncLocalBridgeBuilder {
    connect_timeout: Duration,
    init_timeout: Duration,
    addr: SocketAddr,
    pool_size: usize,
}

impl Default for AsyncLocalBridgeBuilder {
    fn default() -> Self {
        Self {
            connect_timeout: DEFAULT_CONNECT_TIMEOUT,
            init_timeout: DEFAULT_INIT_TIMEOUT,
            addr: crate::DEFAULT_SIMULATOR_HOST.parse().unwrap(),
            pool_size: DEFAULT_POOL_SIZE,
        }
    }
}

impl AsyncLocalBridgeBuilder {
    /// Creates a new builder with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the connection timeout for establishing TCP connections.
    #[must_use]
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// Sets the initialization timeout for waiting for the connection pool.
    #[must_use]
    pub fn init_timeout(mut self, timeout: Duration) -> Self {
        self.init_timeout = timeout;
        self
    }

    /// Sets the simulator address.
    #[must_use]
    pub fn addr(mut self, addr: SocketAddr) -> Self {
        self.addr = addr;
        self
    }

    /// Sets the connection pool size.
    #[must_use]
    pub fn pool_size(mut self, size: usize) -> Self {
        self.pool_size = size;
        self
    }

    /// Builds the AsyncLocalBridge, connecting to the simulator.
    pub async fn build(self) -> Result<AsyncLocalBridge, BridgeError> {
        let statistics = Arc::new(StatisticsEngine::new());
        let soap_client = AsyncTcpSoapClient::new(
            self.addr,
            self.connect_timeout,
            self.pool_size,
            statistics.clone(),
        )
        .await?;

        soap_client
            .ensure_pool_initialized(self.init_timeout)
            .await?;

        Ok(AsyncLocalBridge {
            statistics,
            soap_client,
        })
    }
}

/// Async client for interacting with RealFlight simulators via RealFlight Link.
///
/// # Examples
///
/// ```no_run
/// use realflight_bridge::{AsyncBridge, AsyncLocalBridge, ControlInputs};
/// use std::time::Duration;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // Simple: use defaults
///     let bridge = AsyncLocalBridge::new().await?;
///
///     // Or with custom configuration
///     let bridge = AsyncLocalBridge::builder()
///         .connect_timeout(Duration::from_millis(10))
///         .build()
///         .await?;
///
///     // Create sample control inputs
///     let mut inputs = ControlInputs::default();
///     inputs.channels[0] = 0.5; // Neutral aileron
///     inputs.channels[2] = 1.0; // Full throttle
///
///     // Exchange data with the simulator
///     let state = bridge.exchange_data(&inputs).await?;
///     println!("Current airspeed: {:?}", state.airspeed);
///
///     Ok(())
/// }
/// ```
pub struct AsyncLocalBridge {
    statistics: Arc<StatisticsEngine>,
    soap_client: AsyncTcpSoapClient,
}

impl AsyncBridge for AsyncLocalBridge {
    async fn exchange_data(&self, control: &ControlInputs) -> Result<SimulatorState, BridgeError> {
        let body = encode_control_inputs(control);
        let response = self.soap_client.send_action("ExchangeData", &body).await?;
        match response.status_code {
            200 => crate::decoders::decode_simulator_state(&response.body),
            _ => Err(BridgeError::SoapFault(response.fault_message())),
        }
    }

    async fn enable_rc(&self) -> Result<(), BridgeError> {
        self.soap_client
            .send_action("RestoreOriginalControllerDevice", EMPTY_BODY)
            .await?
            .into()
    }

    async fn disable_rc(&self) -> Result<(), BridgeError> {
        self.soap_client
            .send_action("InjectUAVControllerInterface", EMPTY_BODY)
            .await?
            .into()
    }

    async fn reset_aircraft(&self) -> Result<(), BridgeError> {
        self.soap_client
            .send_action("ResetAircraft", EMPTY_BODY)
            .await?
            .into()
    }
}

impl AsyncLocalBridge {
    /// Creates a new AsyncLocalBridge with default settings.
    pub async fn new() -> Result<Self, BridgeError> {
        AsyncLocalBridgeBuilder::default().build().await
    }

    /// Returns a builder for custom configuration.
    pub fn builder() -> AsyncLocalBridgeBuilder {
        AsyncLocalBridgeBuilder::default()
    }

    /// Returns a snapshot of current statistics.
    pub fn statistics(&self) -> Statistics {
        self.statistics.snapshot()
    }
}
