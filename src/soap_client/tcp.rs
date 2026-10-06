//! Provides and implementation of a SOAP client that uses the TCP protocol.

use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{SocketAddr, TcpStream},
    sync::Arc,
    time::Duration,
};

use crate::BridgeError;
use crate::StatisticsEngine;

use super::http::{Next, ResponseParser, build_http_request};
use super::pool::ConnectionPool;
use super::{SoapClient, SoapResponse, encode_envelope};

/// Implementation of a SOAP client for RealFlight Link that uses the TCP protocol.
pub(crate) struct TcpSoapClient {
    /// Statistics engine for tracking performance
    pub(crate) statistics: Arc<StatisticsEngine>,
    /// Connection pool for managing TCP connections
    pub(crate) connection_pool: ConnectionPool,
}

impl SoapClient for TcpSoapClient {
    /// Sends a SOAP action to the simulator and returns the response.
    ///
    /// # Arguments
    /// * `action` - The SOAP action to send.
    /// * `body`   - The body of the SOAP request.
    fn send_action(&self, action: &str, body: &str) -> Result<SoapResponse, BridgeError> {
        let envelope = encode_envelope(action, body);
        let mut stream = self.connection_pool.get_connection()?;
        self.send_request(&mut stream, action, &envelope)?;
        self.statistics.increment_request_count();

        self.read_response(&mut BufReader::new(stream))
    }
}

impl TcpSoapClient {
    /// Creates a new TCP SOAP client.
    pub fn new(
        addr: SocketAddr,
        connect_timeout: Duration,
        pool_size: usize,
        statistics: Arc<StatisticsEngine>,
    ) -> Result<Self, BridgeError> {
        let connection_pool =
            ConnectionPool::new(addr, connect_timeout, pool_size, statistics.clone())?;
        Ok(TcpSoapClient {
            statistics,
            connection_pool,
        })
    }

    /// Ensures the connection pool is initialized.
    pub(crate) fn ensure_pool_initialized(
        &self,
        init_timeout: Duration,
    ) -> Result<(), BridgeError> {
        self.connection_pool.ensure_initialized(init_timeout)
    }

    /// Sends a request to the simulator.
    fn send_request(
        &self,
        stream: &mut TcpStream,
        action: &str,
        envelope: &str,
    ) -> Result<(), BridgeError> {
        let request = build_http_request(action, envelope);
        stream.write_all(request.as_bytes())?;
        stream.flush()?;
        Ok(())
    }

    /// Reads the raw response from the simulator.
    fn read_response(
        &self,
        stream: &mut BufReader<TcpStream>,
    ) -> Result<SoapResponse, BridgeError> {
        let mut parser = ResponseParser::new();
        let mut line = String::new();
        let length = loop {
            line.clear();
            stream.read_line(&mut line)?;
            if let Next::Body(length) = parser.feed_line(&line)? {
                break length;
            }
        };

        let mut body = vec![0; length];
        stream.read_exact(&mut body)?;

        Ok(parser.finish(body))
    }
}
