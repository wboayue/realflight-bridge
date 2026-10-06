//! Provides and implementation of a SOAP client that uses the TCP protocol.

use std::io::{BufRead, BufReader, Read, Write};

use crate::BridgeError;

use super::http::{Next, ResponseParser};
use super::pool::ConnectionPool;
use super::{SoapClient, SoapResponse, encode_request};

/// Implementation of a SOAP client for RealFlight Link that uses the TCP protocol.
pub(crate) struct TcpSoapClient {
    connection_pool: ConnectionPool,
}

impl SoapClient for TcpSoapClient {
    fn send_action(&self, action: &str, body: &str) -> Result<SoapResponse, BridgeError> {
        let request = encode_request(action, body);
        let mut stream = self.connection_pool.get_connection()?;

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
    /// Creates a TCP SOAP client sending over connections from `connection_pool`.
    pub fn new(connection_pool: ConnectionPool) -> Self {
        TcpSoapClient { connection_pool }
    }
}
