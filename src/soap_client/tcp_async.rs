//! Async implementation of a SOAP client that uses the TCP protocol.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

use crate::BridgeError;
use crate::StatisticsEngine;

use super::http::{Next, ResponseParser};
use super::pool_async::AsyncConnectionPool;
use super::{AsyncSoapClient, SoapResponse, encode_request};

/// Async implementation of a SOAP client for RealFlight Link that uses the TCP protocol.
pub(crate) struct AsyncTcpSoapClient {
    connection_pool: AsyncConnectionPool,
}

impl AsyncSoapClient for AsyncTcpSoapClient {
    async fn send_action(&self, action: &str, body: &str) -> Result<SoapResponse, BridgeError> {
        let request = encode_request(action, body);
        let mut stream = self.connection_pool.get_connection().await?;

        // Send request
        stream.write_all(request.as_bytes()).await?;
        stream.flush().await?;

        self.connection_pool.statistics().increment_request_count();

        // Read response
        let mut reader = BufReader::new(stream);
        let mut parser = ResponseParser::new();
        let mut line = String::new();
        let length = loop {
            line.clear();
            reader.read_line(&mut line).await?;
            if let Next::Body(length) = parser.feed_line(&line)? {
                break length;
            }
        };

        let mut body = vec![0; length];
        reader.read_exact(&mut body).await?;

        Ok(parser.finish(body))
    }
}

impl AsyncTcpSoapClient {
    /// Creates a new async TCP SOAP client.
    pub async fn new(
        addr: SocketAddr,
        connect_timeout: Duration,
        pool_size: usize,
        statistics: Arc<StatisticsEngine>,
    ) -> Result<Self, BridgeError> {
        let connection_pool =
            AsyncConnectionPool::new(addr, connect_timeout, pool_size, statistics).await?;
        Ok(AsyncTcpSoapClient { connection_pool })
    }

    /// Ensures the connection pool is initialized.
    pub async fn ensure_pool_initialized(&self, init_timeout: Duration) -> Result<(), BridgeError> {
        self.connection_pool.ensure_initialized(init_timeout).await
    }

    /// Returns a reference to the statistics engine.
    #[allow(dead_code)]
    pub fn statistics(&self) -> &Arc<StatisticsEngine> {
        self.connection_pool.statistics()
    }
}

#[cfg(test)]
mod tests;
