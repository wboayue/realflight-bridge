use crate::BridgeError;
use crate::decoders::extract_element;

#[cfg(test)]
pub(crate) mod stub;
pub(crate) mod tcp;

/// Response from a SOAP request to the RealFlight simulator
#[derive(Debug)]
pub(crate) struct SoapResponse {
    pub status_code: u32,
    pub body: String,
}

impl SoapResponse {
    /// Extract fault message from a failed SOAP response
    pub fn fault_message(&self) -> String {
        match extract_element("detail", &self.body) {
            Some(message) => message,
            None => "Failed to extract error message".into(),
        }
    }
}

impl From<SoapResponse> for Result<(), BridgeError> {
    fn from(val: SoapResponse) -> Self {
        match val.status_code {
            200 => Ok(()),
            _ => Err(BridgeError::SoapFault(val.fault_message())),
        }
    }
}

/// Trait for sending SOAP requests to the RealFlight simulator
pub(crate) trait SoapClient: Send {
    fn send_action(&self, action: &str, body: &str) -> Result<SoapResponse, BridgeError>;
    #[cfg(test)]
    fn requests(&self) -> Vec<String> {
        Vec::new()
    }
}

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

    mod soap_response_tests {
        use super::*;

        #[test]
        fn fault_message_extracts_detail() {
            let response = SoapResponse {
                status_code: 500,
                body: "<soap:Fault><detail>Error details here</detail></soap:Fault>".to_string(),
            };

            assert_eq!(response.fault_message(), "Error details here");
        }

        #[test]
        fn fault_message_returns_default_when_no_detail() {
            let response = SoapResponse {
                status_code: 500,
                body: "<soap:Fault><faultcode>Client</faultcode></soap:Fault>".to_string(),
            };

            assert_eq!(response.fault_message(), "Failed to extract error message");
        }

        #[test]
        fn converts_200_to_ok() {
            let response = SoapResponse {
                status_code: 200,
                body: String::new(),
            };

            let result: Result<(), BridgeError> = response.into();
            assert!(result.is_ok());
        }

        #[test]
        fn converts_500_to_soap_fault_error() {
            let response = SoapResponse {
                status_code: 500,
                body: "<detail>Server error</detail>".to_string(),
            };

            let result: Result<(), BridgeError> = response.into();
            match result {
                Err(BridgeError::SoapFault(msg)) => {
                    assert_eq!(msg, "Server error");
                }
                other => panic!("expected SoapFault, got {:?}", other),
            }
        }
    }
}
