use super::*;

#[test]
fn serves_responses_in_order() {
    let stub = StubSoapClient::new(&["reset-aircraft-200", "return-data-500"]);

    let first = stub.send_action("ResetAircraft", "").unwrap();
    let second = stub.send_action("ExchangeData", "").unwrap();

    assert_eq!(first.status_code, 200);
    assert!(first.body.contains("ResetAircraftResponse"));
    assert_eq!(second.status_code, 500);
}

#[test]
fn returns_error_when_no_responses() {
    let stub = StubSoapClient::new(&[]);

    assert!(matches!(
        stub.send_action("ResetAircraft", ""),
        Err(BridgeError::Protocol(_))
    ));
}

#[test]
fn records_envelopes() {
    let stub = StubSoapClient::new(&["reset-aircraft-200", "reset-aircraft-200"]);

    stub.send_action("ResetAircraft", "").unwrap();
    stub.send_action("ExchangeData", "<a>1</a>").unwrap();

    assert_eq!(
        stub.requests(),
        [
            encode_envelope("ResetAircraft", ""),
            encode_envelope("ExchangeData", "<a>1</a>"),
        ]
    );
}

#[cfg(feature = "rt-tokio")]
#[tokio::test]
async fn async_send_action_shares_queue() {
    use crate::soap_client::AsyncSoapClient;

    let stub = StubSoapClient::new(&["reset-aircraft-200", "return-data-500"]);

    let first = AsyncSoapClient::send_action(&stub, "ResetAircraft", "")
        .await
        .unwrap();
    let second = SoapClient::send_action(&stub, "ExchangeData", "").unwrap();

    assert_eq!(first.status_code, 200);
    assert_eq!(second.status_code, 500);
    assert_eq!(stub.requests().len(), 2);
}
