use rand::Rng;

use super::*;
use soap_stub::Server;

fn create_configuration(port: u16) -> Configuration {
    Configuration {
        simulator_url: format!("127.0.0.1:{}", port),
        connect_timeout: Duration::from_millis(1000),
        buffer_size: 1,
        ..Default::default()
    }
}

fn create_bridge(port: u16) -> RealFlightBridge {
    let configuration = create_configuration(port);
    RealFlightBridge::new(configuration).unwrap()
}

/// Generate a random port number. Mitigates chances of port conflicts.
fn random_port() -> u16 {
    let mut rng = rand::thread_rng();
    10_000 + rng.gen_range(1..1000)
}

#[test]
pub fn test_reset_aircraft() {
    let port: u16 = random_port();

    let server = Server::new(port, vec!["reset-aircraft-200".to_string()]);

    let bridge = create_bridge(port);

    let result = bridge.reset_aircraft();

    assert!(result.is_ok());
    assert_eq!(server.request_count(), 1);

    let requests = server.requests();

    let reset_request = "\
    <?xml version='1.0' encoding='UTF-8'?><soap:Envelope xmlns:soap='http://schemas.xmlsoap.org/soap/envelope/' xmlns:xsd='http://www.w3.org/2001/XMLSchema' xmlns:xsi='http://www.w3.org/2001/XMLSchema-instance'><soap:Body><ResetAircraft></ResetAircraft></soap:Body></soap:Envelope>\
    ";

    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0], reset_request);

    let statistics = bridge.statistics();

    assert_eq!(statistics.request_count, 1);
    //    assert_eq!(statistics.error_count, 0);
}

#[test]
pub fn test_disable_rc() {
    let port: u16 = random_port();

    let server = Server::new(
        port,
        vec![
            "inject-uav-controller-interface-200".to_string(),
            "inject-uav-controller-interface-500".to_string(),
        ],
    );

    let bridge = create_bridge(port);

    let result = bridge.disable_rc();
    assert!(result.is_ok());

    let _result2 = bridge.disable_rc();
    assert!(result.is_ok());

    let requests = server.requests();

    let disable_request = "\
    <?xml version='1.0' encoding='UTF-8'?><soap:Envelope xmlns:soap='http://schemas.xmlsoap.org/soap/envelope/' xmlns:xsd='http://www.w3.org/2001/XMLSchema' xmlns:xsi='http://www.w3.org/2001/XMLSchema-instance'><soap:Body><InjectUAVControllerInterface></InjectUAVControllerInterface></soap:Body></soap:Envelope>\
    ";

    assert_eq!(server.request_count(), 1);
    assert_eq!(requests[0], disable_request);

    let statistics = bridge.statistics();

    assert_eq!(statistics.request_count, 2);
    //    assert_eq!(statistics.error_count, 0);
}

#[test]
pub fn test_enable_rc() {
    let port: u16 = random_port();

    let server = Server::new(
        port,
        vec![
            "restore-original-controller-device-200".to_string(),
            "restore-original-controller-device-500".to_string(),
        ],
    );

    let configuration = create_configuration(port);
    let bridge = RealFlightBridge::new(configuration).unwrap();

    let result = bridge.enable_rc();
    assert!(result.is_ok());

    let _result2 = bridge.enable_rc();
    assert!(result.is_ok());

    let requests = server.requests();

    let disable_request = "\
    <?xml version='1.0' encoding='UTF-8'?><soap:Envelope xmlns:soap='http://schemas.xmlsoap.org/soap/envelope/' xmlns:xsd='http://www.w3.org/2001/XMLSchema' xmlns:xsi='http://www.w3.org/2001/XMLSchema-instance'><soap:Body><RestoreOriginalControllerDevice></RestoreOriginalControllerDevice></soap:Body></soap:Envelope>\
    ";

    assert_eq!(server.request_count(), 1);
    assert_eq!(requests[0], disable_request);
    //    assert_eq!(requests[1], disable_request); FIXME

    let statistics = bridge.statistics();

    assert_eq!(statistics.request_count, 2);
    //    assert_eq!(statistics.error_count, 0);
}

#[test]
pub fn test_exchange_data_200() {
    // Test the exchange_data method with a successful response from the simulator.

    let port: u16 = random_port();

    let server = Server::new(port, vec!["return-data-200".into()]);

    let bridge = create_bridge(port);

    let mut control = ControlInputs::default();
    for i in 0..control.channels.len() {
        control.channels[i] = i as f32 / 12.0;
    }

    let result = bridge.exchange_data(&control);
    assert!(result.is_ok());

    let requests = server.requests();

    assert_eq!(server.request_count(), 1);
    let control_inputs = "\
    <?xml version='1.0' encoding='UTF-8'?><soap:Envelope xmlns:soap='http://schemas.xmlsoap.org/soap/envelope/' xmlns:xsd='http://www.w3.org/2001/XMLSchema' xmlns:xsi='http://www.w3.org/2001/XMLSchema-instance'><soap:Body><ExchangeData><pControlInputs><m-selectedChannels>4095</m-selectedChannels><m-channelValues-0to1><item>0</item><item>0.083333336</item><item>0.16666667</item><item>0.25</item><item>0.33333334</item><item>0.41666666</item><item>0.5</item><item>0.5833333</item><item>0.6666667</item><item>0.75</item><item>0.8333333</item><item>0.9166667</item></m-channelValues-0to1></pControlInputs></ExchangeData></soap:Body></soap:Envelope>\
    ";
    assert_eq!(requests[0], control_inputs);

    let statistics = bridge.statistics();

    assert_eq!(statistics.request_count, 1);
    assert_eq!(statistics.error_count, 0);
}

#[test]
pub fn test_exchange_data_500() {
    // Test the exchange_data method with a failed response from the simulator.

    let port: u16 = random_port();

    let server = Server::new(port, vec!["return-data-500".into()]);

    let bridge = create_bridge(port);

    let mut control = ControlInputs::default();
    for i in 0..control.channels.len() {
        control.channels[i] = i as f32 / 12.0;
    }

    let result = bridge.exchange_data(&control);
    if let Err(ref e) = result {
        assert_eq!(
            format!("{}", e),
            "RealFlight Link controller has not been instantiated"
        );
    } else {
        panic!("expected error from bridge.exchange_data");
    }

    let requests = server.requests();

    assert_eq!(server.request_count(), 1);
    let control_inputs = "\
    <?xml version='1.0' encoding='UTF-8'?><soap:Envelope xmlns:soap='http://schemas.xmlsoap.org/soap/envelope/' xmlns:xsd='http://www.w3.org/2001/XMLSchema' xmlns:xsi='http://www.w3.org/2001/XMLSchema-instance'><soap:Body><ExchangeData><pControlInputs><m-selectedChannels>4095</m-selectedChannels><m-channelValues-0to1><item>0</item><item>0.083333336</item><item>0.16666667</item><item>0.25</item><item>0.33333334</item><item>0.41666666</item><item>0.5</item><item>0.5833333</item><item>0.6666667</item><item>0.75</item><item>0.8333333</item><item>0.9166667</item></m-channelValues-0to1></pControlInputs></ExchangeData></soap:Body></soap:Envelope>\
    ";
    assert_eq!(requests[0], control_inputs);

    let statistics = bridge.statistics();

    assert_eq!(statistics.request_count, 1);
    // assert_eq!(statistics.error_count, 0);
}

#[cfg(test)]
pub mod soap_stub;
