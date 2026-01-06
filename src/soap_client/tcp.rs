//! Provides and implementation of a SOAP client that uses the TCP protocol.

use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpStream,
    sync::Arc,
};

use crate::BridgeError;
use crate::StatisticsEngine;
use crate::bridge::local::Configuration;

use super::{SoapClient, SoapResponse, encode_envelope};
use super::pool::ConnectionPool;
use super::xml::{build_http_request, parse_status_line, parse_content_length, create_response};

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
        configuration: Configuration,
        statistics: Arc<StatisticsEngine>,
    ) -> Result<Self, BridgeError> {
        let connection_pool = ConnectionPool::new(configuration, statistics.clone())?;
        Ok(TcpSoapClient {
            statistics,
            connection_pool,
        })
    }

    pub(crate) fn ensure_pool_initialized(&self) -> Result<(), BridgeError> {
        self.connection_pool.ensure_pool_initialized()?;
        Ok(())
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
        let mut status_line = String::new();
        stream.read_line(&mut status_line)?;

        let status_code = parse_status_line(&status_line)?;

        // Read headers
        let mut content_length: Option<usize> = None;
        loop {
            let mut line = String::new();
            stream.read_line(&mut line)?;
            if line == "\r\n" {
                break; // End of headers
            }
            if let Some(length) = parse_content_length(&line) {
                content_length = Some(length);
            }
        }

        // Read the body based on Content-Length
        let length = content_length
            .ok_or_else(|| BridgeError::SoapFault("Missing Content-Length header".into()))?;
        let mut body = vec![0; length];
        stream.read_exact(&mut body)?;

        Ok(create_response(status_code, body))
    }
}
