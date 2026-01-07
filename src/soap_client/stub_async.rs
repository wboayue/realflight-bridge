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
mod tests {
    use super::*;

    #[tokio::test]
    async fn returns_queued_response() {
        let stub = AsyncStubSoapClient::new();
        stub.queue_response(SoapResponse {
            status_code: 200,
            body: "test response".to_string(),
        })
        .await;

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
        })
        .await;
        stub.queue_response(SoapResponse {
            status_code: 201,
            body: "second".to_string(),
        })
        .await;

        let first = stub.send_action("Action1", "").await.unwrap();
        let second = stub.send_action("Action2", "").await.unwrap();

        assert_eq!(first.body, "first");
        assert_eq!(second.body, "second");
    }

    #[tokio::test]
    async fn records_requests() {
        let stub = AsyncStubSoapClient::new();
        stub.queue_response(SoapResponse {
            status_code: 200,
            body: "response".to_string(),
        })
        .await;

        let _ = stub.send_action("TestAction", "test body").await;

        let requests = stub.requests().await;
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0], "TestAction:test body");
    }
}
