use crate::BridgeError;
use crate::decoders::extract_element;

#[cfg(feature = "rt-tokio")]
use std::future::Future;

#[cfg(test)]
pub(crate) mod stub;
pub(crate) mod pool;
pub(crate) mod tcp;
pub(crate) mod xml;

#[cfg(feature = "rt-tokio")]
pub(crate) mod pool_async;
#[cfg(feature = "rt-tokio")]
pub(crate) mod tcp_async;
#[cfg(all(test, feature = "rt-tokio"))]
pub(crate) mod stub_async;

pub(crate) use xml::encode_envelope;

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

/// Async trait for sending SOAP requests to the RealFlight simulator
#[cfg(feature = "rt-tokio")]
pub(crate) trait AsyncSoapClient: Send + Sync {
    fn send_action(
        &self,
        action: &str,
        body: &str,
    ) -> impl Future<Output = Result<SoapResponse, BridgeError>> + Send;
}

#[cfg(test)]
mod tests {
    use super::*;

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
