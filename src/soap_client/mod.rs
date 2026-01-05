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
