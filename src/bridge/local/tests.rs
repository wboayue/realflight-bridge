//! Tests for the RealFlightLocalBridge and related functionality.
//!
//! Organized into submodules:
//! - `bridge_operations`: Tests for enable_rc, disable_rc, reset_aircraft, exchange_data
//! - `configuration`: Tests for Configuration defaults and validation
//! - `tcp_integration`: Integration tests using TCP stub server

use std::net::TcpListener;
use std::time::Duration;

use crate::bridge::RealFlightBridge;
use crate::soap_client::stub::StubSoapClient;
use crate::{
    BridgeError, ControlInputs, DEFAULT_SIMULATOR_HOST, SimulatorState, decode_simulator_state,
};

use super::{Configuration, RealFlightLocalBridge};

// ============================================================================
// Test Fixtures
// ============================================================================

const RETURN_DATA_200: &str = include_str!("../../../testdata/responses/return-data-200.xml");

mod fixtures {
    pub const RESET_AIRCRAFT_REQUEST: &str = "\
        <?xml version='1.0' encoding='UTF-8'?>\
        <soap:Envelope xmlns:soap='http://schemas.xmlsoap.org/soap/envelope/' \
        xmlns:xsd='http://www.w3.org/2001/XMLSchema' \
        xmlns:xsi='http://www.w3.org/2001/XMLSchema-instance'>\
        <soap:Body><ResetAircraft></ResetAircraft></soap:Body></soap:Envelope>";

    pub const DISABLE_RC_REQUEST: &str = "\
        <?xml version='1.0' encoding='UTF-8'?>\
        <soap:Envelope xmlns:soap='http://schemas.xmlsoap.org/soap/envelope/' \
        xmlns:xsd='http://www.w3.org/2001/XMLSchema' \
        xmlns:xsi='http://www.w3.org/2001/XMLSchema-instance'>\
        <soap:Body><InjectUAVControllerInterface></InjectUAVControllerInterface></soap:Body></soap:Envelope>";

    pub const ENABLE_RC_REQUEST: &str = "\
        <?xml version='1.0' encoding='UTF-8'?>\
        <soap:Envelope xmlns:soap='http://schemas.xmlsoap.org/soap/envelope/' \
        xmlns:xsd='http://www.w3.org/2001/XMLSchema' \
        xmlns:xsi='http://www.w3.org/2001/XMLSchema-instance'>\
        <soap:Body><RestoreOriginalControllerDevice></RestoreOriginalControllerDevice></soap:Body></soap:Envelope>";
}

fn stub_bridge(responses: Vec<&str>) -> RealFlightLocalBridge {
    let responses: Vec<String> = responses.into_iter().map(String::from).collect();
    RealFlightLocalBridge::stub(StubSoapClient::new(responses))
}

// ============================================================================
// Configuration Tests
// ============================================================================

mod configuration_tests {
    use super::*;

    #[test]
    fn default_uses_localhost() {
        let config = Configuration::default();
        assert_eq!(config.simulator_host, DEFAULT_SIMULATOR_HOST);
    }

    #[test]
    fn default_pool_size_is_one() {
        let config = Configuration::default();
        assert_eq!(config.pool_size, 1);
    }

    #[test]
    fn default_connect_timeout_is_5ms() {
        let config = Configuration::default();
        assert_eq!(config.connect_timeout, Duration::from_millis(5));
    }

    #[test]
    fn configuration_is_cloneable() {
        let config = Configuration {
            simulator_host: "192.168.1.100:18083".to_string(),
            connect_timeout: Duration::from_millis(100),
            pool_size: 5,
        };
        let cloned = config.clone();
        assert_eq!(cloned.simulator_host, config.simulator_host);
        assert_eq!(cloned.connect_timeout, config.connect_timeout);
        assert_eq!(cloned.pool_size, config.pool_size);
    }
}

// ============================================================================
// Bridge Operation Tests
// ============================================================================

mod bridge_operations {
    use super::*;

    #[test]
    fn reset_aircraft_sends_correct_request() {
        let bridge = stub_bridge(vec!["reset-aircraft-200"]);

        let result = bridge.reset_aircraft();
        assert!(result.is_ok());

        let requests = bridge.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0], fixtures::RESET_AIRCRAFT_REQUEST);
    }

    #[test]
    fn reset_aircraft_increments_request_count() {
        let bridge = stub_bridge(vec!["reset-aircraft-200"]);
        bridge.reset_aircraft().unwrap();

        let stats = bridge.statistics();
        assert_eq!(stats.request_count, 1);
    }

    #[test]
    fn disable_rc_sends_correct_request() {
        let bridge = stub_bridge(vec!["inject-uav-controller-interface-200"]);

        let result = bridge.disable_rc();
        assert!(result.is_ok());

        let requests = bridge.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0], fixtures::DISABLE_RC_REQUEST);
    }

    #[test]
    fn disable_rc_returns_soap_fault_on_500() {
        let bridge = stub_bridge(vec!["inject-uav-controller-interface-500"]);

        let result = bridge.disable_rc();
        match result {
            Err(BridgeError::SoapFault(msg)) => {
                assert_eq!(msg, "Preexisting controller reference");
            }
            other => panic!("expected SoapFault, got {:?}", other),
        }
    }

    #[test]
    fn enable_rc_sends_correct_request() {
        let bridge = stub_bridge(vec!["restore-original-controller-device-200"]);

        let result = bridge.enable_rc();
        assert!(result.is_ok());

        let requests = bridge.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0], fixtures::ENABLE_RC_REQUEST);
    }

    #[test]
    fn enable_rc_returns_soap_fault_on_500() {
        let bridge = stub_bridge(vec!["restore-original-controller-device-500"]);

        let result = bridge.enable_rc();
        match result {
            Err(BridgeError::SoapFault(msg)) => {
                assert_eq!(msg, "Pointer to original controller device is null");
            }
            other => panic!("expected SoapFault, got {:?}", other),
        }
    }
}

// ============================================================================
// Exchange Data Tests
// ============================================================================

mod exchange_data {
    use super::*;

    fn create_sequential_inputs() -> ControlInputs {
        let mut control = ControlInputs::default();
        for i in 0..control.channels.len() {
            control.channels[i] = i as f32 / 12.0;
        }
        control
    }

    #[test]
    fn returns_simulator_state_on_success() {
        let bridge = stub_bridge(vec!["return-data-200"]);
        let control = create_sequential_inputs();

        let result = bridge.exchange_data(&control);
        assert!(result.is_ok());

        let state = result.unwrap();
        assert_eq!(state.current_physics_speed_multiplier, 1.0);
    }

    #[test]
    fn returns_soap_fault_on_500() {
        let bridge = stub_bridge(vec!["return-data-500"]);
        let control = create_sequential_inputs();

        let result = bridge.exchange_data(&control);
        match result {
            Err(BridgeError::SoapFault(msg)) => {
                assert_eq!(msg, "RealFlight Link controller has not been instantiated");
            }
            other => panic!("expected SoapFault, got {:?}", other),
        }
    }

    #[test]
    fn returns_decoded_state() {
        let bridge = stub_bridge(vec!["return-data-200"]);

        let state = bridge.exchange_data(&ControlInputs::default()).unwrap();

        let expected = decode_simulator_state(RETURN_DATA_200).unwrap();
        assert_ne!(expected, SimulatorState::default());
        assert_eq!(state, expected);
    }
}

// ============================================================================
// TCP Integration Tests
// ============================================================================

mod tcp_integration {
    use super::*;
    use crate::tests::soap_stub::Server;

    fn get_available_port() -> u16 {
        TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port()
    }

    fn create_bridge(port: u16) -> Result<RealFlightLocalBridge, BridgeError> {
        let config = Configuration {
            simulator_host: format!("127.0.0.1:{}", port),
            connect_timeout: Duration::from_millis(1000),
            pool_size: 1,
        };
        RealFlightLocalBridge::with_configuration(&config)
    }

    #[test]
    fn tcp_client_sends_and_receives() {
        let port = get_available_port();
        let server = Server::new(port, vec!["reset-aircraft-200".to_string()]);
        let bridge = create_bridge(port).unwrap();

        let result = bridge.reset_aircraft();
        assert!(result.is_ok(), "expected Ok: {:?}", result);

        let stats = bridge.statistics();
        assert_eq!(stats.request_count, 1);
        assert_eq!(stats.error_count, 0);

        drop(server);
    }
}
