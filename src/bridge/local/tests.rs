//! Tests for the RealFlightLocalBridge and related functionality.
//!
//! Organized into submodules:
//! - `bridge_operations`: Tests for enable_rc, disable_rc, reset_aircraft, exchange_data
//! - `configuration`: Tests for Configuration defaults and validation
//! - `tcp_integration`: Integration tests using TCP stub server

use std::time::Duration;

use crate::bridge::RealFlightBridge;
use crate::soap_client::encode_envelope;
use crate::soap_client::stub::StubSoapClient;
use crate::{
    BridgeError, ControlInputs, DEFAULT_SIMULATOR_HOST, SimulatorState, decode_simulator_state,
};

use super::{Configuration, RealFlightLocalBridge};

// ============================================================================
// Test Fixtures
// ============================================================================

const RETURN_DATA_200: &str = include_str!("../../../testdata/responses/return-data-200.xml");

fn stub_bridge(responses: &[&str]) -> RealFlightLocalBridge {
    RealFlightLocalBridge::stub(StubSoapClient::new(responses))
}

/// SOAP envelopes received by a stub bridge.
fn requests(bridge: &RealFlightLocalBridge) -> Vec<String> {
    bridge.session.client.as_stub().unwrap().requests()
}

// ============================================================================
// Configuration Tests
// ============================================================================

mod configuration {
    use super::*;

    #[test]
    fn default_uses_localhost() {
        let config = Configuration::default();
        assert_eq!(config.simulator_host, DEFAULT_SIMULATOR_HOST);
    }

    #[test]
    fn default_pool_size_is_zero() {
        let config = Configuration::default();
        assert_eq!(config.pool_size, 0);
    }

    #[test]
    fn default_connect_timeout_is_5ms() {
        let config = Configuration::default();
        assert_eq!(config.connect_timeout, Duration::from_millis(50));
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

    #[test]
    fn invalid_host_fails_fast() {
        let config = Configuration {
            simulator_host: "not-a-valid-socket-addr".to_string(),
            ..Configuration::default()
        };

        match RealFlightLocalBridge::with_configuration(&config) {
            Err(BridgeError::Initialization(msg)) => {
                assert!(msg.contains("Invalid simulator host"));
            }
            Err(other) => panic!("expected Initialization error, got {:?}", other),
            Ok(_) => panic!("expected Initialization error, got Ok"),
        }
    }
}

// ============================================================================
// Bridge Operation Tests
// ============================================================================

mod bridge_operations {
    use super::*;

    type UnitOp = fn(&RealFlightLocalBridge) -> Result<(), BridgeError>;

    #[test]
    fn unit_ops_send_correct_requests() {
        let cases: [(UnitOp, &str, &str); 3] = [
            (
                RealFlightLocalBridge::enable_rc,
                "restore-original-controller-device-200",
                "RestoreOriginalControllerDevice",
            ),
            (
                RealFlightLocalBridge::disable_rc,
                "inject-uav-controller-interface-200",
                "InjectUAVControllerInterface",
            ),
            (
                RealFlightLocalBridge::reset_aircraft,
                "reset-aircraft-200",
                "ResetAircraft",
            ),
        ];

        for (op, response, action) in cases {
            let bridge = stub_bridge(&[response]);
            op(&bridge).unwrap();
            assert_eq!(requests(&bridge), [encode_envelope(action, "")]);
        }
    }

    #[test]
    fn counts_requests_and_failures() {
        let bridge = stub_bridge(&["reset-aircraft-200", "inject-uav-controller-interface-500"]);
        bridge.reset_aircraft().unwrap();
        bridge.disable_rc().unwrap_err();
        // Stub exhausted: send fails
        bridge.enable_rc().unwrap_err();

        let stats = bridge.statistics();
        assert_eq!(stats.request_count, 3);
        assert_eq!(stats.error_count, 2);
    }

    #[test]
    fn returns_soap_fault_on_500() {
        let bridge = stub_bridge(&["inject-uav-controller-interface-500"]);

        match bridge.disable_rc() {
            Err(BridgeError::SoapFault(msg)) => {
                assert_eq!(msg, "Preexisting controller reference");
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

    #[test]
    fn returns_decoded_state() {
        let bridge = stub_bridge(&["return-data-200"]);

        let state = bridge.exchange_data(&ControlInputs::default()).unwrap();

        let expected = decode_simulator_state(RETURN_DATA_200).unwrap();
        assert_ne!(expected, SimulatorState::default());
        assert_eq!(state, expected);
    }

    #[test]
    fn returns_soap_fault_on_500() {
        let bridge = stub_bridge(&["return-data-500"]);

        match bridge.exchange_data(&ControlInputs::default()) {
            Err(BridgeError::SoapFault(msg)) => {
                assert_eq!(msg, "RealFlight Link controller has not been instantiated");
            }
            other => panic!("expected SoapFault, got {:?}", other),
        }
    }
}

// ============================================================================
// TCP Integration Tests
// ============================================================================

mod tcp_integration {
    use super::*;
    use crate::soap_client::test_support::Server;

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
        let server = Server::new(&["reset-aircraft-200"]);
        let bridge = create_bridge(server.port()).unwrap();

        let result = bridge.reset_aircraft();
        assert!(result.is_ok(), "expected Ok: {:?}", result);

        let stats = bridge.statistics();
        assert_eq!(stats.request_count, 1);
        assert_eq!(stats.error_count, 0);

        drop(server);
    }
}
