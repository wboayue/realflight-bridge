//! State shared by sync and async local bridges: the SOAP client and statistics.

use std::sync::Arc;

use crate::soap_client::Client;
use crate::{BridgeError, Statistics, StatisticsEngine};

#[cfg(test)]
use crate::soap_client::stub::StubSoapClient;

/// A local bridge's SOAP client and the statistics for calls made through it.
pub(super) struct Session<T> {
    pub(super) client: Client<T>,
    statistics: Arc<StatisticsEngine>,
}

impl<T> Session<T> {
    /// `statistics` is shared with the client's connection pool, which records
    /// failed connection attempts.
    pub(super) fn new(client: T, statistics: Arc<StatisticsEngine>) -> Self {
        Session {
            client: Client::Tcp(client),
            statistics,
        }
    }

    /// Creates a session backed by a stub SOAP client (no network).
    #[cfg(test)]
    pub(super) fn stub(stub: StubSoapClient) -> Self {
        Session {
            client: Client::Stub(stub),
            statistics: Arc::new(StatisticsEngine::new()),
        }
    }

    /// Records a completed call, counting it as an error if it failed.
    pub(super) fn record<R>(&self, result: Result<R, BridgeError>) -> Result<R, BridgeError> {
        self.statistics.increment_request_count();
        if result.is_err() {
            self.statistics.increment_error_count();
        }
        result
    }

    /// Returns a snapshot of current statistics.
    pub(super) fn statistics(&self) -> Statistics {
        self.statistics.snapshot()
    }
}
