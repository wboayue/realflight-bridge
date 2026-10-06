//! Async implementation of a SOAP client that uses the TCP protocol.

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

use crate::BridgeError;

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
    /// Creates an async TCP SOAP client sending over connections from `connection_pool`.
    pub fn new(connection_pool: AsyncConnectionPool) -> Self {
        AsyncTcpSoapClient { connection_pool }
    }
}

#[cfg(test)]
mod tests;
