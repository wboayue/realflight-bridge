//! Builder for [RealFlightLocalBridge], and the settings and setters it shares
//! with the async builder.

use std::{net::SocketAddr, sync::Arc, time::Duration};

use crate::defaults;
use crate::soap_client::pool::ConnectionPool;
use crate::soap_client::tcp::TcpSoapClient;
use crate::timeout::ensure_nonzero;
use crate::{BridgeError, StatisticsEngine};

use super::RealFlightLocalBridge;
use super::session::Session;

#[cfg(test)]
mod tests;

/// Settings shared by the sync and async local bridge builders.
#[derive(Debug, Clone)]
pub(super) struct Settings {
    pub(super) connect_timeout: Duration,
    pub(super) init_timeout: Duration,
    pub(super) request_timeout: Duration,
    pub(super) addr: SocketAddr,
    pub(super) pool_size: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            connect_timeout: defaults::CONNECT_TIMEOUT,
            init_timeout: defaults::INIT_TIMEOUT,
            request_timeout: defaults::REQUEST_TIMEOUT,
            addr: crate::DEFAULT_SIMULATOR_HOST.parse().unwrap(),
            pool_size: defaults::POOL_SIZE,
        }
    }
}

impl Settings {
    /// Rejects settings that would fail every request.
    pub(super) fn validate(&self) -> Result<(), BridgeError> {
        ensure_nonzero("request_timeout", self.request_timeout)
            .map_err(|e| BridgeError::Initialization(e.to_string()))
    }
}

/// Setters shared by the sync and async local bridge builders, which hold
/// their [Settings] in a `settings` field.
macro_rules! settings_setters {
    () => {
        /// Sets the connection timeout for establishing TCP connections (default 10 ms).
        #[must_use]
        pub fn connect_timeout(mut self, timeout: std::time::Duration) -> Self {
            self.settings.connect_timeout = timeout;
            self
        }

        /// Sets the initialization timeout for waiting for the connection pool
        /// (default 5 s).
        #[must_use]
        pub fn init_timeout(mut self, timeout: std::time::Duration) -> Self {
            self.settings.init_timeout = timeout;
            self
        }

        /// Sets the time allowed for each request to the simulator (default 50 ms),
        /// from getting a connection to reading the full response. A request that
        /// takes longer fails with [BridgeError::Connection](crate::BridgeError::Connection)
        /// of kind [`TimedOut`](std::io::ErrorKind::TimedOut), so a stalled simulator
        /// can't block the caller. Must be non-zero; `Duration::MAX` disables it.
        #[must_use]
        pub fn request_timeout(mut self, timeout: std::time::Duration) -> Self {
            self.settings.request_timeout = timeout;
            self
        }

        /// Sets the simulator address (default
        /// [`DEFAULT_SIMULATOR_HOST`](crate::DEFAULT_SIMULATOR_HOST)).
        #[must_use]
        pub fn addr(mut self, addr: std::net::SocketAddr) -> Self {
            self.settings.addr = addr;
            self
        }

        /// Sets the connection pool size (default 0).
        ///
        /// 0 connects on demand: each request opens its own connection and the
        /// simulator isn't contacted until the first request. Larger values pre-open
        /// connections to hide connect latency, but newer RealFlight versions stall
        /// while a pre-opened connection sits idle.
        #[must_use]
        pub fn pool_size(mut self, size: usize) -> Self {
            self.settings.pool_size = size;
            self
        }
    };
}
#[cfg_attr(not(feature = "rt-tokio"), allow(unused_imports))]
pub(super) use settings_setters;

/// Builder for [RealFlightLocalBridge].
///
/// Configure options, then call [`build()`](Self::build).
///
/// ```no_run
/// use std::time::Duration;
/// use realflight_bridge::{BridgeError, RealFlightLocalBridge};
///
/// fn main() -> Result<(), BridgeError> {
///     let bridge = RealFlightLocalBridge::builder()
///         .request_timeout(Duration::from_millis(20))
///         .build()?;
///     Ok(())
/// }
/// ```
#[derive(Debug, Clone, Default)]
pub struct RealFlightLocalBridgeBuilder {
    pub(super) settings: Settings,
}

impl RealFlightLocalBridgeBuilder {
    /// Creates a new builder with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    settings_setters!();

    /// Builds the bridge. With a non-zero pool size, waits for the pool to
    /// connect; with 0, the simulator is contacted on the first request.
    ///
    /// # Errors
    ///
    /// Fails if the request timeout is zero, or if a non-zero pool can't connect
    /// (e.g., RealFlight is not running).
    pub fn build(self) -> Result<RealFlightLocalBridge, BridgeError> {
        let settings = self.settings;
        settings.validate()?;

        let statistics = Arc::new(StatisticsEngine::new());
        let pool = ConnectionPool::new(
            settings.addr,
            settings.connect_timeout,
            settings.pool_size,
            statistics.clone(),
        )?;
        pool.ensure_initialized(settings.init_timeout)?;

        Ok(RealFlightLocalBridge {
            session: Session::new(
                TcpSoapClient::new(pool, settings.request_timeout),
                statistics,
            ),
        })
    }
}
