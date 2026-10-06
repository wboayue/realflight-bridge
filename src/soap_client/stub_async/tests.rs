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
