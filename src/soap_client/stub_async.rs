//! Provides an async implementation of a SOAP client that returns stubbed responses.
//! Useful for testing.

use std::collections::VecDeque;

use tokio::sync::Mutex;

use crate::BridgeError;

use super::{AsyncSoapClient, SoapResponse};

/// Async stub SOAP client for testing.
pub(crate) struct AsyncStubSoapClient {
    responses: Mutex<VecDeque<SoapResponse>>,
    requests: Mutex<Vec<String>>,
}

impl AsyncStubSoapClient {
    /// Creates a new stub client with no queued responses.
    #[allow(dead_code)]
    pub fn new() -> Self {
        AsyncStubSoapClient {
            responses: Mutex::new(VecDeque::new()),
            requests: Mutex::new(Vec::new()),
        }
    }

    /// Returns recorded requests (action and body).
    #[allow(dead_code)]
    pub async fn requests(&self) -> Vec<String> {
        self.requests.lock().await.clone()
    }

    /// Queues a response to be returned by the next call to `send_action`.
    #[allow(dead_code)]
    pub async fn queue_response(&self, response: SoapResponse) {
        let mut responses = self.responses.lock().await;
        responses.push_back(response);
    }
}

impl AsyncSoapClient for AsyncStubSoapClient {
    async fn send_action(&self, action: &str, body: &str) -> Result<SoapResponse, BridgeError> {
        // Record the request
        let request = format!("{}:{}", action, body);
        self.requests.lock().await.push(request);

        let mut responses = self.responses.lock().await;
        responses
            .pop_front()
            .ok_or_else(|| BridgeError::SoapFault("No more stubbed responses available".into()))
    }
}

#[cfg(test)]
mod tests;
