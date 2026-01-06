//! Provides an async implementation of a SOAP client that returns stubbed responses.
//! Useful for testing.

use std::collections::VecDeque;
use std::sync::Mutex;

use crate::BridgeError;

use super::{AsyncSoapClient, SoapResponse};

/// Async stub SOAP client for testing.
pub(crate) struct AsyncStubSoapClient {
    responses: Mutex<VecDeque<SoapResponse>>,
}

impl AsyncStubSoapClient {
    /// Creates a new stub client with no queued responses.
    #[allow(dead_code)]
    pub fn new() -> Self {
        AsyncStubSoapClient {
            responses: Mutex::new(VecDeque::new()),
        }
    }

    /// Queues a response to be returned by the next call to `send_action`.
    #[allow(dead_code)]
    pub fn queue_response(&self, response: SoapResponse) {
        let mut responses = self.responses.lock().unwrap();
        responses.push_back(response);
    }
}

impl AsyncSoapClient for AsyncStubSoapClient {
    async fn send_action(&self, _action: &str, _body: &str) -> Result<SoapResponse, BridgeError> {
        let mut responses = self.responses.lock().unwrap();
        responses.pop_front().ok_or_else(|| {
            BridgeError::SoapFault("No more stubbed responses available".into())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn returns_queued_response() {
        let stub = AsyncStubSoapClient::new();
        stub.queue_response(SoapResponse {
            status_code: 200,
            body: "test response".to_string(),
        });

        let response = stub.send_action("TestAction", "").await.unwrap();
        assert_eq!(response.status_code, 200);
        assert_eq!(response.body, "test response");
    }

    #[tokio::test]
    async fn returns_error_when_no_responses() {
        let stub = AsyncStubSoapClient::new();
        let result = stub.send_action("TestAction", "").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn returns_responses_in_order() {
        let stub = AsyncStubSoapClient::new();
        stub.queue_response(SoapResponse {
            status_code: 200,
            body: "first".to_string(),
        });
        stub.queue_response(SoapResponse {
            status_code: 201,
            body: "second".to_string(),
        });

        let first = stub.send_action("Action1", "").await.unwrap();
        let second = stub.send_action("Action2", "").await.unwrap();

        assert_eq!(first.body, "first");
        assert_eq!(second.body, "second");
    }
}
