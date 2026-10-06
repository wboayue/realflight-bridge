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
            return Err(BridgeError::SoapFault(
                "Connection closed while reading headers".into(),
            ));
        }

        if line == "\r\n" {
            return self
                .content_length
                .map(Next::Body)
                .ok_or_else(|| BridgeError::SoapFault("Missing Content-Length header".into()));
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
        return Err(BridgeError::SoapFault(
            "Empty response from simulator".into(),
        ));
    }

    status_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| {
            BridgeError::SoapFault("Malformed HTTP status line: missing status code".into())
        })?
        .parse()
        .map_err(|e| BridgeError::SoapFault(format!("Invalid HTTP status code: {}", e)))
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
mod tests {
    use super::*;

    mod build_http_request_tests {
        use super::*;

        #[test]
        fn includes_post_method() {
            let request = build_http_request("Test", "body");
            assert!(request.starts_with("POST / HTTP/1.1\r\n"));
        }

        #[test]
        fn includes_soapaction_header() {
            let request = build_http_request("MyAction", "body");
            assert!(request.contains("Soapaction: 'MyAction'\r\n"));
        }

        #[test]
        fn includes_content_length_header() {
            let request = build_http_request("Test", "hello");
            assert!(request.contains("Content-Length: 5\r\n"));
        }

        #[test]
        fn includes_content_type_header() {
            let request = build_http_request("Test", "body");
            assert!(request.contains("Content-Type: text/xml;charset=utf-8\r\n"));
        }

        #[test]
        fn ends_with_envelope() {
            let request = build_http_request("Test", "<envelope>data</envelope>");
            assert!(request.ends_with("<envelope>data</envelope>"));
        }
    }

    mod response_parser_tests {
        use super::*;

        /// Feeds lines until the parser asks for the body.
        fn parse_head(lines: &[&str]) -> Result<(ResponseParser, usize), BridgeError> {
            let mut parser = ResponseParser::new();
            for line in lines {
                if let Next::Body(len) = parser.feed_line(line)? {
                    return Ok((parser, len));
                }
            }
            panic!("parser did not reach body");
        }

        #[test]
        fn parses_200_response() {
            let (parser, len) =
                parse_head(&["HTTP/1.1 200 OK\r\n", "Content-Length: 4\r\n", "\r\n"]).unwrap();
            assert_eq!(len, 4);

            let response = parser.finish(b"<a/>".to_vec());
            assert_eq!(response.status_code, 200);
            assert_eq!(response.body, "<a/>");
        }

        #[test]
        fn parses_500_response() {
            let (parser, _) = parse_head(&[
                "HTTP/1.1 500 Internal Server Error\r\n",
                "Content-Length: 7\r\n",
                "\r\n",
            ])
            .unwrap();

            let response = parser.finish(b"<Fault>".to_vec());
            assert_eq!(response.status_code, 500);
            assert!(response.body.contains("Fault"));
        }

        #[test]
        fn ignores_other_headers() {
            let (_, len) = parse_head(&[
                "HTTP/1.1 200 OK\r\n",
                "Content-Type: text/xml\r\n",
                "content-length: 12\r\n",
                "Connection: close\r\n",
                "\r\n",
            ])
            .unwrap();
            assert_eq!(len, 12);
        }

        #[test]
        fn missing_content_length_returns_error() {
            match parse_head(&["HTTP/1.1 200 OK\r\n", "\r\n"]) {
                Err(BridgeError::SoapFault(msg)) => assert!(msg.contains("Content-Length")),
                other => panic!("expected SoapFault, got {:?}", other.map(|(_, l)| l)),
            }
        }

        #[test]
        fn eof_before_status_returns_error() {
            match parse_head(&[""]) {
                Err(BridgeError::SoapFault(msg)) => assert!(msg.contains("Empty response")),
                other => panic!("expected SoapFault, got {:?}", other.map(|(_, l)| l)),
            }
        }

        #[test]
        fn eof_in_headers_returns_error() {
            match parse_head(&["HTTP/1.1 200 OK\r\n", "Content-Length: 4\r\n", ""]) {
                Err(BridgeError::SoapFault(msg)) => assert!(msg.contains("Connection closed")),
                other => panic!("expected SoapFault, got {:?}", other.map(|(_, l)| l)),
            }
        }

        #[test]
        fn malformed_status_returns_error() {
            assert!(parse_head(&["INVALID\r\n"]).is_err());
        }
    }

    mod parse_status_line_tests {
        use super::*;

        #[test]
        fn parses_200_ok() {
            let status = parse_status_line("HTTP/1.1 200 OK\r\n").unwrap();
            assert_eq!(status, 200);
        }

        #[test]
        fn parses_500_error() {
            let status = parse_status_line("HTTP/1.1 500 Internal Server Error\r\n").unwrap();
            assert_eq!(status, 500);
        }

        #[test]
        fn errors_on_empty_line() {
            let result = parse_status_line("");
            assert!(result.is_err());
        }

        #[test]
        fn errors_on_malformed_line() {
            let result = parse_status_line("INVALID");
            assert!(result.is_err());
        }

        #[test]
        fn errors_on_invalid_status_code() {
            let result = parse_status_line("HTTP/1.1 NOT_A_NUMBER OK");
            assert!(result.is_err());
            match result {
                Err(BridgeError::SoapFault(msg)) => {
                    assert!(msg.contains("Invalid HTTP status code"));
                }
                other => panic!("expected SoapFault, got {:?}", other),
            }
        }
    }

    mod parse_content_length_tests {
        use super::*;

        #[test]
        fn extracts_content_length() {
            let length = parse_content_length("Content-Length: 1234");
            assert_eq!(length, Some(1234));
        }

        #[test]
        fn case_insensitive() {
            let length = parse_content_length("content-length: 567");
            assert_eq!(length, Some(567));
        }

        #[test]
        fn returns_none_for_other_headers() {
            let length = parse_content_length("Content-Type: text/xml");
            assert_eq!(length, None);
        }
    }
}
