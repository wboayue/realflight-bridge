//! Stub SOAP client returning canned responses. Implements both the sync and
//! async client traits, so it backs tests of either local bridge.

use std::collections::VecDeque;
use std::sync::Mutex;

use crate::BridgeError;

use super::test_support::canned_response;
use super::{SoapClient, SoapResponse, encode_envelope};

pub(crate) struct StubSoapClient {
    responses: Mutex<VecDeque<String>>,
    requests: Mutex<Vec<String>>,
}

impl StubSoapClient {
    /// Serves `testdata/responses/{key}.xml` for each key, in order.
    pub fn new(keys: &[&str]) -> Self {
        StubSoapClient {
            responses: Mutex::new(keys.iter().map(|key| key.to_string()).collect()),
            requests: Mutex::new(Vec::new()),
        }
    }

    /// Returns the SOAP envelopes received so far.
    pub fn requests(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }

    fn respond(&self, action: &str, body: &str) -> Result<SoapResponse, BridgeError> {
        self.requests
            .lock()
            .unwrap()
            .push(encode_envelope(action, body));

        let key = self
            .responses
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| BridgeError::Protocol("No more stubbed responses".into()))?;
        Ok(canned_response(&key))
    }
}

impl SoapClient for StubSoapClient {
    fn send_action(&self, action: &str, body: &str) -> Result<SoapResponse, BridgeError> {
        self.respond(action, body)
    }
}

#[cfg(feature = "rt-tokio")]
impl super::AsyncSoapClient for StubSoapClient {
    async fn send_action(&self, action: &str, body: &str) -> Result<SoapResponse, BridgeError> {
        self.respond(action, body)
    }
}

#[cfg(test)]
mod tests;
