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

/// The SOAP client a local bridge talks through: the TCP client `T`, or a stub in tests.
pub(crate) enum Client<T> {
    Tcp(T),
    #[cfg(test)]
    Stub(stub::StubSoapClient),
}

impl<T> Client<T> {
    /// Returns the envelopes received by a stub client.
    #[cfg(test)]
    pub(crate) fn requests(&self) -> Vec<String> {
        match self {
            Client::Stub(stub) => stub.requests(),
            Client::Tcp(_) => panic!("requests() needs a stub client"),
        }
    }
}

impl<T: SoapClient> SoapClient for Client<T> {
    fn send_action(&self, action: &str, body: &str) -> Result<SoapResponse, BridgeError> {
        match self {
            Client::Tcp(client) => client.send_action(action, body),
            #[cfg(test)]
            Client::Stub(stub) => SoapClient::send_action(stub, action, body),
        }
    }
}

#[cfg(feature = "rt-tokio")]
impl<T: AsyncSoapClient> AsyncSoapClient for Client<T> {
    async fn send_action(&self, action: &str, body: &str) -> Result<SoapResponse, BridgeError> {
        match self {
            Client::Tcp(client) => client.send_action(action, body).await,
            #[cfg(test)]
            Client::Stub(stub) => AsyncSoapClient::send_action(stub, action, body).await,
        }
    }
}

#[cfg(test)]
mod tests;
