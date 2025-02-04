use rand::Rng;

use super::*;
use soap_stub::Server;

#[test]
pub fn test_reset_aircraft() {
    let mut rng = rand::thread_rng();
    let port: u16 = 10_000 + rng.gen_range(1..1000);

    let mut server = Server::new(port, vec!["reset-aircraft-200".to_string()]);

    server.setup();

    let configuration = Configuration {
        simulator_url: format!("127.0.0.1:{}", port),
        ..Default::default()
    };
    let bridge = RealFlightBridge::new(configuration).unwrap();

    let result = bridge.reset_aircraft();

    assert!(result.is_ok());
    assert_eq!(server.request_count(), 2);

    let requests = server.requests();

    let reset_request = "\
    POST / HTTP/1.1\r\n\
    Soapaction: 'ResetAircraft'\r\n\
    Content-Length: 277\r\n\
    Content-Type: text/xml;charset=utf-8\r\n\
    Connection: Keep-Alive\r\n\
    \r\n\
    <?xml version='1.0' encoding='UTF-8'?><soap:Envelope xmlns:soap='http://schemas.xmlsoap.org/soap/envelope/' xmlns:xsd='http://www.w3.org/2001/XMLSchema' xmlns:xsi='http://www.w3.org/2001/XMLSchema-instance'><soap:Body><ResetAircraft></ResetAircraft></soap:Body></soap:Envelope>\
    ";

    assert_eq!(requests.len(), 2); // fixeme
    assert_eq!(requests[0], reset_request);

    let statistics = bridge.statistics();

    assert_eq!(statistics.request_count, 1);
    assert_eq!(statistics.error_count, 0);
}

#[test]
pub fn test_disable_rc() {
    let mut rng = rand::thread_rng();
    let port: u16 = 10_000 + rng.gen_range(1..1000);

    let mut server = Server::new(
        port,
        vec![
            "inject-uav-controller-interface-200".to_string(),
            "inject-uav-controller-interface-500".to_string(),
        ],
    );

    server.setup();

    let configuration = Configuration {
        simulator_url: format!("127.0.0.1:{}", port),
        ..Default::default()
    };
    let bridge = RealFlightBridge::new(configuration).unwrap();

    let result = bridge.disable_rc();
    assert!(result.is_ok());

    let result2 = bridge.disable_rc();
    assert!(result.is_ok());

    let requests = server.requests();

    let disable_request = "\
    POST / HTTP/1.1\r\n\
    Soapaction: 'InjectUAVControllerInterface'\r\n\
    Content-Length: 307\r\n\
    Content-Type: text/xml;charset=utf-8\r\n\
    Connection: Keep-Alive\r\n\
    \r\n\
    <?xml version='1.0' encoding='UTF-8'?><soap:Envelope xmlns:soap='http://schemas.xmlsoap.org/soap/envelope/' xmlns:xsd='http://www.w3.org/2001/XMLSchema' xmlns:xsi='http://www.w3.org/2001/XMLSchema-instance'><soap:Body><InjectUAVControllerInterface></InjectUAVControllerInterface></soap:Body></soap:Envelope>\
    ";

    assert_eq!(server.request_count(), 2); // FIXME
    assert_eq!(requests[0], disable_request);
    //    assert_eq!(requests[1], disable_request); FIXME

    let statistics = bridge.statistics();

    assert_eq!(statistics.request_count, 2);
    assert_eq!(statistics.error_count, 0);
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
