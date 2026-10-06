use crate::BridgeError;
use crate::decoders::extract_element;

#[cfg(feature = "rt-tokio")]
use std::future::Future;

pub(crate) mod http;
pub(crate) mod pool;
#[cfg(test)]
pub(crate) mod stub;
pub(crate) mod tcp;
#[cfg(test)]
pub(crate) mod test_support;
pub(crate) mod xml;

#[cfg(feature = "rt-tokio")]
pub(crate) mod pool_async;
#[cfg(all(test, feature = "rt-tokio"))]
pub(crate) mod stub_async;
#[cfg(feature = "rt-tokio")]
pub(crate) mod tcp_async;

pub(crate) use xml::encode_envelope;

/// Encodes a complete HTTP request for a SOAP action.
pub(crate) fn encode_request(action: &str, body: &str) -> String {
    http::build_http_request(action, &encode_envelope(action, body))
}

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
mod tests;
