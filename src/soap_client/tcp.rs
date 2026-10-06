//! Provides and implementation of a SOAP client that uses the TCP protocol.

use std::{
    io::{BufRead, BufReader, Read, Write},
    net::SocketAddr,
    sync::Arc,
    time::Duration,
};

use crate::BridgeError;
use crate::StatisticsEngine;

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
    /// Creates a new TCP SOAP client. `statistics` records connection errors.
    pub fn new(
        addr: SocketAddr,
        connect_timeout: Duration,
        pool_size: usize,
        statistics: Arc<StatisticsEngine>,
    ) -> Result<Self, BridgeError> {
        let connection_pool = ConnectionPool::new(addr, connect_timeout, pool_size, statistics)?;
        Ok(TcpSoapClient { connection_pool })
    }

    /// Ensures the connection pool is initialized.
    pub fn ensure_pool_initialized(&self, init_timeout: Duration) -> Result<(), BridgeError> {
        self.connection_pool.ensure_initialized(init_timeout)
    }
}
