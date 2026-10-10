//! Async implementation of the local bridge for RealFlight simulator.

use std::sync::Arc;

use crate::bridge::AsyncBridge;
use crate::soap_client::pool_async::AsyncConnectionPool;
use crate::soap_client::tcp_async::AsyncTcpSoapClient;
use crate::soap_client::{AsyncSoapClient, SoapResponse};
use crate::{BridgeError, ControlInputs, SimulatorState, Statistics, StatisticsEngine};

use super::builder::{Settings, settings_setters};
use super::ops::{Op, decode_exchange, decode_unit};
use super::session::Session;

/// Builder for AsyncLocalBridge.
///
/// Configure options synchronously, then call `build()`.
#[derive(Debug, Clone, Default)]
pub struct AsyncLocalBridgeBuilder {
    settings: Settings,
}

impl AsyncLocalBridgeBuilder {
    /// Creates a new builder with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    settings_setters!();

    /// Builds the AsyncLocalBridge. With a non-zero pool size, waits for the pool
    /// to connect; with 0, the simulator is contacted on the first request.
    /// Fails if the request timeout is zero.
    pub async fn build(self) -> Result<AsyncLocalBridge, BridgeError> {
        let settings = self.settings;
        settings.validate()?;

        let statistics = Arc::new(StatisticsEngine::new());
        let pool = AsyncConnectionPool::new(
            settings.addr,
            settings.connect_timeout,
            settings.pool_size,
            statistics.clone(),
        )
        .await?;
        pool.ensure_initialized(settings.init_timeout).await?;

        Ok(AsyncLocalBridge {
            session: Session::new(
                AsyncTcpSoapClient::new(pool, settings.request_timeout),
                statistics,
            ),
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
