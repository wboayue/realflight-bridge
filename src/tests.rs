use super::*;
use soap_stub::{MockResponse, Server};

#[test]
pub fn test_activate() {
    let mut server = Server::new(vec![
            MockResponse {
                response: "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<SOAP-ENV:Envelope xmlns:SOAP-ENV=\"http://schemas.xmlsoap.org/soap/envelope/\" xmlns:SOAP-ENC=\"http://schemas.xmlsoap.org/soap/encoding/\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:xsd=\"http://www.w3.org/2001/XMLSchema\"><SOAP-ENV:Body><ResetAircraftResponse><unused>0</unused></ResetAircraftResponse></SOAP-ENV:Body></SOAP-ENV:Envelope>"
            },
            MockResponse {
                response: "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<SOAP-ENV:Envelope xmlns:SOAP-ENV=\"http://schemas.xmlsoap.org/soap/envelope/\" xmlns:SOAP-ENC=\"http://schemas.xmlsoap.org/soap/encoding/\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:xsd=\"http://www.w3.org/2001/XMLSchema\"><SOAP-ENV:Body><InjectUAVControllerInterfaceResponse><unused>0</unused></InjectUAVControllerInterfaceResponse></SOAP-ENV:Body></SOAP-ENV:Envelope>",
            }
        ]
    );

    server.setup();

    let configuration = Configuration::default();

    let bridge = RealFlightBridge::new(configuration).unwrap();
    bridge.activate().unwrap();

    let statistics = bridge.statistics();

    assert_eq!(statistics.request_count, 2);
    assert_eq!(statistics.error_count, 0);

    // assert_eq!(server.requests()[0], "Activate");
    // assert_eq!(server.requests()[1], "InjectUAVControllerInterface");
}

#[test]
pub fn test_encode_control_inputs() {
    let soap_body = "<pControlInputs><m-selectedChannels>4095</m-selectedChannels><m-channelValues-0to1><item>0</item><item>0</item><item>0</item><item>0</item><item>0</item><item>0</item><item>0</item><item>0</item><item>0</item><item>0</item><item>0</item><item>0</item></m-channelValues-0to1></pControlInputs>";
    let control = ControlInputs::default();
    assert_eq!(encode_control_inputs(&control), soap_body);
}

#[test]
pub fn test_encode_envelope() {
    let envelope = "<?xml version='1.0' encoding='UTF-8'?><soap:Envelope xmlns:soap='http://schemas.xmlsoap.org/soap/envelope/' xmlns:xsd='http://www.w3.org/2001/XMLSchema' xmlns:xsi='http://www.w3.org/2001/XMLSchema-instance'><soap:Body><InjectUAVControllerInterface></InjectUAVControllerInterface></soap:Body></soap:Envelope>";
    assert_eq!(
        encode_envelope("InjectUAVControllerInterface", UNUSED),
        envelope
    );
}

#[cfg(test)]
mod soap_stub;

