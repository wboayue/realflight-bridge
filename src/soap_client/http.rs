//! HTTP request building and incremental response parsing for SOAP calls.
//! This module is runtime-agnostic (no I/O); sync and async clients feed it lines.

use super::SoapResponse;
use crate::BridgeError;

/// Size of header for request body
const HEADER_LEN: usize = 120;

/// Build an HTTP request string for a SOAP action
pub(crate) fn build_http_request(action: &str, envelope: &str) -> String {
    let mut request = String::with_capacity(HEADER_LEN + envelope.len() + action.len());

    request.push_str("POST / HTTP/1.1\r\n");
    request.push_str(&format!("Soapaction: '{}'\r\n", action));
    request.push_str(&format!("Content-Length: {}\r\n", envelope.len()));
    request.push_str("Content-Type: text/xml;charset=utf-8\r\n");
    request.push_str("\r\n");
    request.push_str(envelope);

    request
}

/// What the caller should read next.
#[derive(Debug, PartialEq)]
pub(crate) enum Next {
    /// Read another line and feed it to the parser.
    NeedLine,
    /// Headers done; read exactly this many body bytes and call [ResponseParser::finish].
    Body(usize),
}

/// Incremental parser for an HTTP response head (status line + headers).
///
/// Feed each line as read, including its trailing `\r\n`. An empty line
/// signals EOF (what `read_line` yields when the peer closes).
#[derive(Debug, Default)]
pub(crate) struct ResponseParser {
    status_code: Option<u32>,
    content_length: Option<usize>,
}

impl ResponseParser {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn feed_line(&mut self, line: &str) -> Result<Next, BridgeError> {
        if self.status_code.is_none() {
            self.status_code = Some(parse_status_line(line)?);
            return Ok(Next::NeedLine);
        }

        if line.is_empty() {
            return Err(BridgeError::Protocol(
                "Connection closed while reading headers".into(),
            ));
        }

        if line == "\r\n" {
            return self
                .content_length
                .map(Next::Body)
                .ok_or_else(|| BridgeError::Protocol("Missing Content-Length header".into()));
        }

        if let Some(length) = parse_content_length(line) {
            self.content_length = Some(length);
        }
        Ok(Next::NeedLine)
    }

    /// Builds the response once the body has been read.
    ///
    /// Must only be called after [ResponseParser::feed_line] returned [Next::Body].
    pub(crate) fn finish(self, body: Vec<u8>) -> SoapResponse {
        SoapResponse {
            status_code: self
                .status_code
                .expect("finish called before status line was parsed"),
            body: String::from_utf8_lossy(&body).to_string(),
        }
    }
}

/// Parse HTTP status line and extract status code
fn parse_status_line(status_line: &str) -> Result<u32, BridgeError> {
    if status_line.is_empty() {
        return Err(BridgeError::Protocol(
            "Empty response from simulator".into(),
        ));
    }

    status_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| {
            BridgeError::Protocol("Malformed HTTP status line: missing status code".into())
        })?
        .parse()
        .map_err(|e| BridgeError::Protocol(format!("Invalid HTTP status code: {}", e)))
}

/// Extract Content-Length from a header line if present
fn parse_content_length(line: &str) -> Option<usize> {
    const PREFIX: &str = "content-length:";
    if line.len() >= PREFIX.len() && line[..PREFIX.len()].eq_ignore_ascii_case(PREFIX) {
        line.split_whitespace()
            .nth(1)
            .and_then(|s| s.trim().parse().ok())
    } else {
        None
    }
}

#[cfg(test)]
mod tests;
