//! Async implementation of a SOAP client that uses the TCP protocol.

use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::time::timeout;

use crate::BridgeError;
use crate::timeout::timed_out;

use super::http::{Next, ResponseParser};
use super::pool_async::AsyncConnectionPool;
use super::{AsyncSoapClient, SoapResponse, encode_request};

/// Async implementation of a SOAP client for RealFlight Link that uses the TCP protocol.
pub(crate) struct AsyncTcpSoapClient {
    connection_pool: AsyncConnectionPool,
    request_timeout: Duration,
}

impl AsyncSoapClient for AsyncTcpSoapClient {
    /// Fails with `TimedOut` if the full exchange, including getting a
    /// connection, takes longer than the request timeout.
    async fn send_action(&self, action: &str, body: &str) -> Result<SoapResponse, BridgeError> {
        timeout(self.request_timeout, self.exchange(action, body))
            .await
            .map_err(|_| timed_out("simulator", self.request_timeout))?
    }
}

impl AsyncTcpSoapClient {
    /// Creates an async TCP SOAP client sending over connections from
    /// `connection_pool`, bounding each request by `request_timeout`.
    pub fn new(connection_pool: AsyncConnectionPool, request_timeout: Duration) -> Self {
        AsyncTcpSoapClient {
            connection_pool,
            request_timeout,
        }
    }

    /// Sends one request and reads its response, without a time bound.
    async fn exchange(&self, action: &str, body: &str) -> Result<SoapResponse, BridgeError> {
        let request = encode_request(action, body);
        let mut stream = self.connection_pool.get_connection().await?;

        // Send request
        stream.write_all(request.as_bytes()).await?;
        stream.flush().await?;

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

#[cfg(test)]
mod tests;
