//! Provides and implementation of a SOAP client that uses the TCP protocol.

use std::io::{self, BufRead, BufReader, ErrorKind, Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

use crate::BridgeError;

use super::http::{Next, ResponseParser};
use super::pool::ConnectionPool;
use super::{SoapClient, SoapResponse, encode_request};

#[cfg(test)]
mod tests;

/// Implementation of a SOAP client for RealFlight Link that uses the TCP protocol.
pub(crate) struct TcpSoapClient {
    connection_pool: ConnectionPool,
    request_timeout: Duration,
}

impl SoapClient for TcpSoapClient {
    /// Fails with `TimedOut` if the full exchange, including getting a
    /// connection, takes longer than the request timeout.
    fn send_action(&self, action: &str, body: &str) -> Result<SoapResponse, BridgeError> {
        let deadline = Instant::now() + self.request_timeout;
        self.exchange(action, body, deadline).map_err(|e| match e {
            BridgeError::Connection(e)
                if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) =>
            {
                BridgeError::Connection(io::Error::new(
                    ErrorKind::TimedOut,
                    format!(
                        "no response from simulator within {:?}",
                        self.request_timeout
                    ),
                ))
            }
            e => e,
        })
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

    /// Sets the time allowed for each request.
    pub fn set_request_timeout(&mut self, request_timeout: Duration) {
        self.request_timeout = request_timeout;
    }

    /// Sends one request and reads its response, failing once `deadline` passes.
    fn exchange(
        &self,
        action: &str,
        body: &str,
        deadline: Instant,
    ) -> Result<SoapResponse, BridgeError> {
        let request = encode_request(action, body);
        let mut stream = DeadlineStream {
            stream: self.connection_pool.get_connection(remaining(deadline)?)?,
            deadline,
        };

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

/// Time left before `deadline`, or `TimedOut` once it has passed.
fn remaining(deadline: Instant) -> io::Result<Duration> {
    let left = deadline.saturating_duration_since(Instant::now());
    if left.is_zero() {
        return Err(io::Error::from(ErrorKind::TimedOut));
    }
    Ok(left)
}

/// Stream whose reads and writes share one deadline: each call's socket
/// timeout is set to the time left.
struct DeadlineStream {
    stream: TcpStream,
    deadline: Instant,
}

impl Read for DeadlineStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.stream
            .set_read_timeout(Some(remaining(self.deadline)?))?;
        self.stream.read(buf)
    }
}

impl Write for DeadlineStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.stream
            .set_write_timeout(Some(remaining(self.deadline)?))?;
        self.stream.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }
}
