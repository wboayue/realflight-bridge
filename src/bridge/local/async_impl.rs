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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::AsyncBridge;
    use crate::tests::soap_stub::Server;
    use approx::assert_relative_eq;
    use std::net::TcpListener;

    fn get_available_port() -> u16 {
        TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port()
    }

    async fn create_bridge(port: u16) -> Result<AsyncLocalBridge, BridgeError> {
        let addr: SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();
        AsyncLocalBridge::builder()
            .addr(addr)
            .connect_timeout(Duration::from_millis(1000))
            .init_timeout(Duration::from_secs(5))
            .build()
            .await
    }

    // ========================================================================
    // Builder Tests
    // ========================================================================

    mod builder_tests {
        use super::*;

        #[test]
        fn builder_default_connect_timeout() {
            let builder = AsyncLocalBridgeBuilder::new();
            assert_eq!(builder.connect_timeout, Duration::from_millis(5));
        }

        #[test]
        fn builder_default_init_timeout() {
            let builder = AsyncLocalBridgeBuilder::new();
            assert_eq!(builder.init_timeout, Duration::from_secs(5));
        }

        #[test]
        fn builder_default_pool_size() {
            let builder = AsyncLocalBridgeBuilder::new();
            assert_eq!(builder.pool_size, 1);
        }

        #[test]
        fn builder_connect_timeout_sets_value() {
            let builder =
                AsyncLocalBridgeBuilder::new().connect_timeout(Duration::from_millis(100));
            assert_eq!(builder.connect_timeout, Duration::from_millis(100));
        }

        #[test]
        fn builder_init_timeout_sets_value() {
            let builder = AsyncLocalBridgeBuilder::new().init_timeout(Duration::from_secs(10));
            assert_eq!(builder.init_timeout, Duration::from_secs(10));
        }

        #[test]
        fn builder_addr_sets_value() {
            let addr: SocketAddr = "192.168.1.100:18083".parse().unwrap();
            let builder = AsyncLocalBridgeBuilder::new().addr(addr);
            assert_eq!(builder.addr, addr);
        }

        #[test]
        fn builder_pool_size_sets_value() {
            let builder = AsyncLocalBridgeBuilder::new().pool_size(5);
            assert_eq!(builder.pool_size, 5);
        }

        #[test]
        fn builder_is_cloneable() {
            let builder = AsyncLocalBridgeBuilder::new()
                .connect_timeout(Duration::from_millis(100))
                .pool_size(3);
            let cloned = builder.clone();
            assert_eq!(cloned.connect_timeout, builder.connect_timeout);
            assert_eq!(cloned.pool_size, builder.pool_size);
        }
    }

    // ========================================================================
    // Bridge Operation Tests (TCP Integration)
    // ========================================================================

    mod bridge_operations {
        use super::*;

        #[tokio::test]
        async fn reset_aircraft_succeeds() {
            let port = get_available_port();
            let _server = Server::new(port, vec!["reset-aircraft-200".to_string()]);
            let bridge = create_bridge(port).await.unwrap();

            let result = bridge.reset_aircraft().await;
            assert!(result.is_ok(), "expected Ok: {:?}", result);
        }

        #[tokio::test]
        async fn reset_aircraft_increments_request_count() {
            let port = get_available_port();
            let _server = Server::new(port, vec!["reset-aircraft-200".to_string()]);
            let bridge = create_bridge(port).await.unwrap();

            bridge.reset_aircraft().await.unwrap();

            let stats = bridge.statistics();
            assert_eq!(stats.request_count, 1);
        }

        #[tokio::test]
        async fn enable_rc_succeeds() {
            let port = get_available_port();
            let _server = Server::new(
                port,
                vec!["restore-original-controller-device-200".to_string()],
            );
            let bridge = create_bridge(port).await.unwrap();

            let result = bridge.enable_rc().await;
            assert!(result.is_ok(), "expected Ok: {:?}", result);
        }

        #[tokio::test]
        async fn enable_rc_returns_soap_fault_on_500() {
            let port = get_available_port();
            let _server = Server::new(
                port,
                vec!["restore-original-controller-device-500".to_string()],
            );
            let bridge = create_bridge(port).await.unwrap();

            let result = bridge.enable_rc().await;
            match result {
                Err(BridgeError::SoapFault(msg)) => {
                    assert_eq!(msg, "Pointer to original controller device is null");
                }
                other => panic!("expected SoapFault, got {:?}", other),
            }
        }

        #[tokio::test]
        async fn disable_rc_succeeds() {
            let port = get_available_port();
            let _server = Server::new(
                port,
                vec!["inject-uav-controller-interface-200".to_string()],
            );
            let bridge = create_bridge(port).await.unwrap();

            let result = bridge.disable_rc().await;
            assert!(result.is_ok(), "expected Ok: {:?}", result);
        }

        #[tokio::test]
        async fn disable_rc_returns_soap_fault_on_500() {
            let port = get_available_port();
            let _server = Server::new(
                port,
                vec!["inject-uav-controller-interface-500".to_string()],
            );
            let bridge = create_bridge(port).await.unwrap();

            let result = bridge.disable_rc().await;
            match result {
                Err(BridgeError::SoapFault(msg)) => {
                    assert_eq!(msg, "Preexisting controller reference");
                }
                other => panic!("expected SoapFault, got {:?}", other),
            }
        }
    }

    // ========================================================================
    // Exchange Data Tests
    // ========================================================================

    mod exchange_data {
        use super::*;

        #[tokio::test]
        async fn returns_simulator_state_on_success() {
            let port = get_available_port();
            let _server = Server::new(port, vec!["return-data-200".to_string()]);
            let bridge = create_bridge(port).await.unwrap();

            let control = ControlInputs::default();
            let result = bridge.exchange_data(&control).await;

            assert!(result.is_ok(), "expected Ok: {:?}", result);
            let state = result.unwrap();
            assert_eq!(state.current_physics_speed_multiplier, 1.0);
        }

        #[tokio::test]
        async fn returns_soap_fault_on_500() {
            let port = get_available_port();
            let _server = Server::new(port, vec!["return-data-500".to_string()]);
            let bridge = create_bridge(port).await.unwrap();

            let control = ControlInputs::default();
            let result = bridge.exchange_data(&control).await;

            match result {
                Err(BridgeError::SoapFault(msg)) => {
                    assert_eq!(msg, "RealFlight Link controller has not been instantiated");
                }
                other => panic!("expected SoapFault, got {:?}", other),
            }
        }

        #[tokio::test]
        async fn parses_boolean_fields() {
            let port = get_available_port();
            let _server = Server::new(port, vec!["return-data-200".to_string()]);
            let bridge = create_bridge(port).await.unwrap();

            let state = bridge
                .exchange_data(&ControlInputs::default())
                .await
                .unwrap();

            assert!(!state.is_locked);
            assert!(!state.has_lost_components);
            assert!(state.an_engine_is_running);
            assert!(!state.is_touching_ground);
            assert!(state.flight_axis_controller_is_active);
        }

        #[cfg(not(feature = "uom"))]
        #[tokio::test]
        async fn parses_velocity_fields() {
            let port = get_available_port();
            let _server = Server::new(port, vec!["return-data-200".to_string()]);
            let bridge = create_bridge(port).await.unwrap();

            let state = bridge
                .exchange_data(&ControlInputs::default())
                .await
                .unwrap();

            assert_relative_eq!(state.airspeed, 0.040872246);
            assert_relative_eq!(state.groundspeed, 4.643444754E-06);
        }
    }

    // ========================================================================
    // Statistics Tests
    // ========================================================================

    mod statistics_tests {
        use super::*;

        #[tokio::test]
        async fn statistics_returns_snapshot() {
            let port = get_available_port();
            let _server = Server::new(
                port,
                vec![
                    "reset-aircraft-200".to_string(),
                    "reset-aircraft-200".to_string(),
                ],
            );
            let bridge = create_bridge(port).await.unwrap();

            bridge.reset_aircraft().await.unwrap();
            bridge.reset_aircraft().await.unwrap();

            let stats = bridge.statistics();
            assert_eq!(stats.request_count, 2);
            assert_eq!(stats.error_count, 0);
        }
    }
}
