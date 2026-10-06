//! Async implementation of the local bridge for RealFlight simulator.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use crate::bridge::AsyncBridge;
use crate::defaults;
use crate::soap_client::pool_async::AsyncConnectionPool;
use crate::soap_client::tcp_async::AsyncTcpSoapClient;
use crate::soap_client::{AsyncSoapClient, SoapResponse};
use crate::{BridgeError, ControlInputs, SimulatorState, Statistics, StatisticsEngine};

use super::ops::{Op, decode_exchange, decode_unit};
use super::session::Session;

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
            connect_timeout: defaults::CONNECT_TIMEOUT,
            init_timeout: defaults::INIT_TIMEOUT,
            addr: crate::DEFAULT_SIMULATOR_HOST.parse().unwrap(),
            pool_size: defaults::POOL_SIZE,
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
    ///
    /// 0 disables pre-connecting: each request opens its own connection, so no
    /// idle connection is held open (slightly higher latency per request). The
    /// simulator isn't contacted until the first request.
    #[must_use]
    pub fn pool_size(mut self, size: usize) -> Self {
        self.pool_size = size;
        self
    }

    /// Builds the AsyncLocalBridge, connecting to the simulator.
    pub async fn build(self) -> Result<AsyncLocalBridge, BridgeError> {
        let statistics = Arc::new(StatisticsEngine::new());
        let pool = AsyncConnectionPool::new(
            self.addr,
            self.connect_timeout,
            self.pool_size,
            statistics.clone(),
        )
        .await?;
        pool.ensure_initialized(self.init_timeout).await?;

        Ok(AsyncLocalBridge {
            session: Session::new(AsyncTcpSoapClient::new(pool), statistics),
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
///     println!("Current airspeed: {:?}", state.airspeed_mps);
///
///     Ok(())
/// }
/// ```
pub struct AsyncLocalBridge {
    session: Session<AsyncTcpSoapClient>,
}

impl AsyncBridge for AsyncLocalBridge {
    async fn exchange_data(&self, control: &ControlInputs) -> Result<SimulatorState, BridgeError> {
        self.call(Op::Exchange(control), decode_exchange).await
    }

    async fn enable_rc(&self) -> Result<(), BridgeError> {
        self.call(Op::EnableRc, decode_unit).await
    }

    async fn disable_rc(&self) -> Result<(), BridgeError> {
        self.call(Op::DisableRc, decode_unit).await
    }

    async fn reset_aircraft(&self) -> Result<(), BridgeError> {
        self.call(Op::Reset, decode_unit).await
    }
}

impl AsyncLocalBridge {
    /// Sends an operation to the simulator and decodes the response.
    async fn call<R>(
        &self,
        op: Op<'_>,
        decode: fn(SoapResponse) -> Result<R, BridgeError>,
    ) -> Result<R, BridgeError> {
        let result = self
            .session
            .client
            .send_action(op.action(), &op.body())
            .await
            .and_then(decode);
        self.session.record(result)
    }

    /// Creates a bridge backed by a stub SOAP client (no network).
    #[cfg(test)]
    pub(crate) fn stub(soap_client: crate::soap_client::stub::StubSoapClient) -> Self {
        AsyncLocalBridge {
            session: Session::stub(soap_client),
        }
    }

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
        self.session.statistics()
    }
}

#[cfg(test)]
mod tests;
