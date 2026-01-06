//! Pure functions for SOAP/HTTP request building and response parsing.
//! This module is runtime-agnostic (no I/O).

use crate::BridgeError;
use super::SoapResponse;

/// Size of header for request body
const HEADER_LEN: usize = 120;

/// Encode a SOAP envelope for RealFlight
pub(crate) fn encode_envelope(action: &str, body: &str) -> String {
    let mut envelope = String::with_capacity(200 + body.len());

    envelope.push_str("<?xml version='1.0' encoding='UTF-8'?>");
    envelope.push_str("<soap:Envelope xmlns:soap='http://schemas.xmlsoap.org/soap/envelope/' xmlns:xsd='http://www.w3.org/2001/XMLSchema' xmlns:xsi='http://www.w3.org/2001/XMLSchema-instance'>");
    envelope.push_str("<soap:Body>");
    envelope.push_str(&format!("<{}>{}</{}>", action, body, action));
    envelope.push_str("</soap:Body>");
    envelope.push_str("</soap:Envelope>");

    envelope
}

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

/// Parse HTTP status line and extract status code
pub(crate) fn parse_status_line(status_line: &str) -> Result<u32, BridgeError> {
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
pub(crate) fn parse_content_length(line: &str) -> Option<usize> {
    if line.to_lowercase().starts_with("content-length:") {
        line.split_whitespace()
            .nth(1)
            .and_then(|s| s.trim().parse().ok())
    } else {
        None
    }
}

/// Create a SoapResponse from parsed components
pub(crate) fn create_response(status_code: u32, body: Vec<u8>) -> SoapResponse {
    SoapResponse {
        status_code,
        body: String::from_utf8_lossy(&body).to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod encode_envelope_tests {
        use super::*;

        #[test]
        fn encodes_empty_body() {
            let result = encode_envelope("TestAction", "");

            assert!(result.contains("<?xml version='1.0' encoding='UTF-8'?>"));
            assert!(result.contains("<TestAction></TestAction>"));
            assert!(result.contains("<soap:Body>"));
            assert!(result.contains("</soap:Body>"));
        }

        #[test]
        fn encodes_action_with_body() {
            let body = "<param>value</param>";
            let result = encode_envelope("MyAction", body);

            assert!(result.contains("<MyAction><param>value</param></MyAction>"));
        }

        #[test]
        fn includes_soap_namespaces() {
            let result = encode_envelope("Test", "");

            assert!(result.contains("xmlns:soap='http://schemas.xmlsoap.org/soap/envelope/'"));
            assert!(result.contains("xmlns:xsd='http://www.w3.org/2001/XMLSchema'"));
            assert!(result.contains("xmlns:xsi='http://www.w3.org/2001/XMLSchema-instance'"));
        }

        #[test]
        fn starts_with_xml_declaration() {
            let result = encode_envelope("Test", "");
            assert!(result.starts_with("<?xml version='1.0' encoding='UTF-8'?>"));
        }

        #[test]
        fn ends_with_envelope_close() {
            let result = encode_envelope("Test", "");
            assert!(result.ends_with("</soap:Envelope>"));
        }

        #[test]
        fn encodes_real_actions() {
            // Test actual action names used in the crate
            let actions = [
                ("ResetAircraft", ""),
                ("InjectUAVControllerInterface", ""),
                ("RestoreOriginalControllerDevice", ""),
                ("ExchangeData", "<pControlInputs></pControlInputs>"),
            ];

            for (action, body) in actions {
                let result = encode_envelope(action, body);
                assert!(
                    result.contains(&format!("<{}>", action)),
                    "Missing open tag for {}",
                    action
                );
                assert!(
                    result.contains(&format!("</{}>", action)),
                    "Missing close tag for {}",
                    action
                );
            }
        }
    }

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
