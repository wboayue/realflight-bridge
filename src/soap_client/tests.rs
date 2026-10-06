use super::*;

#[test]
fn encode_request_wraps_envelope_in_http() {
    let envelope = encode_envelope("ResetAircraft", "");
    let request = encode_request("ResetAircraft", "");

    assert!(request.starts_with("POST / HTTP/1.1\r\n"));
    assert!(request.contains("Soapaction: 'ResetAircraft'\r\n"));
    assert!(request.contains(&format!("Content-Length: {}\r\n", envelope.len())));
    assert!(request.ends_with(&envelope));
}

mod soap_response {
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
}
