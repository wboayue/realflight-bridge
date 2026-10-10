//! Provides and implementation of a SOAP client that uses the TCP protocol.

use std::io::{BufRead, BufReader, Read, Write};
use std::time::Duration;

use crate::BridgeError;
use crate::timeout::{Deadline, DeadlineStream};

use super::http::{Next, ResponseParser};
use super::pool::ConnectionPool;
use super::{SoapClient, SoapResponse, encode_request};

#[cfg(test)]
mod tests;

/// Peer name in timeout errors.
const PEER: &str = "simulator";

/// Implementation of a SOAP client for RealFlight Link that uses the TCP protocol.
pub(crate) struct TcpSoapClient {
    connection_pool: ConnectionPool,
    request_timeout: Duration,
}

impl SoapClient for TcpSoapClient {
    /// Fails with `TimedOut` if the full exchange, including getting a
    /// connection, takes longer than the request timeout.
    fn send_action(&self, action: &str, body: &str) -> Result<SoapResponse, BridgeError> {
        let deadline = Deadline::after(self.request_timeout);
        let request = encode_request(action, body);
        let max_wait = deadline.remaining(PEER)?.unwrap_or(Duration::MAX);
        let mut stream = DeadlineStream::new(self.connection_pool.get_connection(max_wait)?, PEER);
        stream.set_deadline(deadline);

        // Send request
        stream.write_all(request.as_bytes())?;
        stream.flush()?;

        // Read response
        let mut reader = BufReader::new(stream);
        let mut parser = ResponseParser::new();
        let mut line = String::new();
        let length = loop {
            line.clear();
            reader.read_line(&mut line)?;
            if let Next::Body(length) = parser.feed_line(&line)? {
                break length;
            }
        };

        let mut body = vec![0; length];
        reader.read_exact(&mut body)?;

        Ok(parser.finish(body))
    }
}

impl TcpSoapClient {
    /// Creates a TCP SOAP client sending over connections from
    /// `connection_pool`, bounding each request by `request_timeout`.
    pub fn new(connection_pool: ConnectionPool, request_timeout: Duration) -> Self {
        TcpSoapClient {
            connection_pool,
            request_timeout,
        }
    }
}
