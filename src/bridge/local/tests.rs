//! Tests for the RealFlightLocalBridge and related functionality.
//!
//! Organized into submodules:
//! - `bridge_operations`: Tests for enable_rc, disable_rc, reset_aircraft, exchange_data
//! - `configuration`: Tests for Configuration defaults and validation
//! - `tcp_integration`: Integration tests using TCP stub server

use std::net::TcpListener;
use std::time::Duration;

use approx::assert_relative_eq;

use crate::bridge::RealFlightBridge;
use crate::soap_client::stub::StubSoapClient;
use crate::{BridgeError, ControlInputs, DEFAULT_SIMULATOR_HOST};

use super::{Configuration, RealFlightLocalBridge};

// ============================================================================
// Test Fixtures
// ============================================================================

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
    fn parses_boolean_fields() {
        let bridge = stub_bridge(vec!["return-data-200"]);
        let control = ControlInputs::default();

        let state = bridge.exchange_data(&control).unwrap();

        assert!(!state.is_locked);
        assert!(!state.has_lost_components);
        assert!(state.an_engine_is_running);
        assert!(!state.is_touching_ground);
        assert!(state.flight_axis_controller_is_active);
    }

    #[test]
    fn parses_string_fields() {
        let bridge = stub_bridge(vec!["return-data-200"]);
        let control = ControlInputs::default();

        let state = bridge.exchange_data(&control).unwrap();
        assert_eq!(state.current_aircraft_status, "CAS-WAITINGTOLAUNCH");
    }

    #[test]
    fn parses_rpm_fields() {
        let bridge = stub_bridge(vec!["return-data-200"]);
        let control = ControlInputs::default();

        let state = bridge.exchange_data(&control).unwrap();
        assert_relative_eq!(state.prop_rpm, 47.404716491);
        assert_relative_eq!(state.heli_main_rotor_rpm, -1.0);
    }

    #[cfg(not(feature = "uom"))]
    mod raw_values {
        use super::*;

        #[test]
        fn parses_velocity_fields() {
            let bridge = stub_bridge(vec!["return-data-200"]);
            let state = bridge.exchange_data(&ControlInputs::default()).unwrap();

            assert_relative_eq!(state.airspeed, 0.040872246);
            assert_relative_eq!(state.groundspeed, 4.643444754E-06);
            assert_relative_eq!(state.velocity_world_u, -2.005582700E-06);
            assert_relative_eq!(state.velocity_world_v, 4.187984814E-06);
            assert_relative_eq!(state.velocity_world_w, 0.040872246);
            assert_relative_eq!(state.velocity_body_u, -0.001089469);
            assert_relative_eq!(state.velocity_body_v, -0.000530726);
            assert_relative_eq!(state.velocity_body_w, 0.040854275);
        }

        #[test]
        fn parses_position_fields() {
            let bridge = stub_bridge(vec!["return-data-200"]);
            let state = bridge.exchange_data(&ControlInputs::default()).unwrap();

            assert_relative_eq!(state.altitude_asl, 1127.370971679);
            assert_relative_eq!(state.altitude_agl, 0.266309916);
            assert_relative_eq!(state.aircraft_position_x, 5575.6806640625);
            assert_relative_eq!(state.aircraft_position_y, 1715.962158203125);
        }

        #[test]
        fn parses_acceleration_fields() {
            let bridge = stub_bridge(vec!["return-data-200"]);
            let state = bridge.exchange_data(&ControlInputs::default()).unwrap();

            assert_relative_eq!(state.acceleration_world_ax, -0.000483050);
            assert_relative_eq!(state.acceleration_world_ay, 0.001008689);
            assert_relative_eq!(state.acceleration_world_az, 9.844209671);
            assert_relative_eq!(state.acceleration_body_ax, -0.000176936);
            assert_relative_eq!(state.acceleration_body_ay, -8.662045001E-05);
            assert_relative_eq!(state.acceleration_body_az, 0.044223785);
        }

        #[test]
        fn parses_battery_fields() {
            let bridge = stub_bridge(vec!["return-data-200"]);
            let state = bridge.exchange_data(&ControlInputs::default()).unwrap();

            assert_relative_eq!(state.battery_voltage, 12.599982261);
            assert_relative_eq!(state.battery_current_draw, 0.0);
            assert_relative_eq!(state.battery_remaining_capacity, 3999.990722656);
            assert_relative_eq!(state.fuel_remaining, -1.0);
        }

        #[test]
        fn parses_angular_rate_fields() {
            let bridge = stub_bridge(vec!["return-data-200"]);
            let state = bridge.exchange_data(&ControlInputs::default()).unwrap();

            assert_relative_eq!(state.pitch_rate, 0.001380353);
            assert_relative_eq!(state.roll_rate, -0.000032227);
            assert_relative_eq!(state.yaw_rate, 0.001473751);
        }

        #[test]
        fn parses_wind_fields() {
            let bridge = stub_bridge(vec!["return-data-200"]);
            let state = bridge.exchange_data(&ControlInputs::default()).unwrap();

            assert_relative_eq!(state.wind_x, 0.0);
            assert_relative_eq!(state.wind_y, 0.0);
            assert_relative_eq!(state.wind_z, 0.0);
        }

        #[test]
        fn parses_time_field() {
            let bridge = stub_bridge(vec!["return-data-200"]);
            let state = bridge.exchange_data(&ControlInputs::default()).unwrap();

            assert_relative_eq!(state.current_physics_time, 72263.411813672);
        }
    }

    #[cfg(feature = "uom")]
    mod with_units {
        use super::*;
        use uom::si::acceleration::meter_per_second_squared;
        use uom::si::angular_velocity::degree_per_second;
        use uom::si::electric_charge::milliampere_hour;
        use uom::si::electric_current::ampere;
        use uom::si::electric_potential::volt;
        use uom::si::length::meter;
        use uom::si::time::second;
        use uom::si::velocity::meter_per_second;
        use uom::si::volume::liter;

        use crate::decoders::OUNCES_PER_LITER;

        #[test]
        fn parses_velocity_fields() {
            let bridge = stub_bridge(vec!["return-data-200"]);
            let state = bridge.exchange_data(&ControlInputs::default()).unwrap();

            assert_relative_eq!(state.airspeed.get::<meter_per_second>(), 0.040872246);
            assert_relative_eq!(state.groundspeed.get::<meter_per_second>(), 4.643444754E-06);
        }

        #[test]
        fn parses_position_fields() {
            let bridge = stub_bridge(vec!["return-data-200"]);
            let state = bridge.exchange_data(&ControlInputs::default()).unwrap();

            assert_relative_eq!(state.altitude_asl.get::<meter>(), 1127.370971679);
            assert_relative_eq!(state.altitude_agl.get::<meter>(), 0.266309916);
        }

        #[test]
        fn parses_acceleration_fields() {
            let bridge = stub_bridge(vec!["return-data-200"]);
            let state = bridge.exchange_data(&ControlInputs::default()).unwrap();

            assert_relative_eq!(
                state
                    .acceleration_world_az
                    .get::<meter_per_second_squared>(),
                9.844209671
            );
        }

        #[test]
        fn parses_battery_fields() {
            let bridge = stub_bridge(vec!["return-data-200"]);
            let state = bridge.exchange_data(&ControlInputs::default()).unwrap();

            assert_relative_eq!(state.battery_voltage.get::<volt>(), 12.599982261);
            assert_relative_eq!(state.battery_current_draw.get::<ampere>(), 0.0);
            assert_relative_eq!(
                state.battery_remaining_capacity.get::<milliampere_hour>(),
                3999.990722656
            );
            assert_relative_eq!(state.fuel_remaining.get::<liter>(), -1.0 / OUNCES_PER_LITER);
        }

        #[test]
        fn parses_angular_rate_fields() {
            let bridge = stub_bridge(vec!["return-data-200"]);
            let state = bridge.exchange_data(&ControlInputs::default()).unwrap();

            assert_relative_eq!(state.pitch_rate.get::<degree_per_second>(), 0.001380353);
            assert_relative_eq!(state.roll_rate.get::<degree_per_second>(), -0.000032227);
            assert_relative_eq!(state.yaw_rate.get::<degree_per_second>(), 0.001473751);
        }

        #[test]
        fn parses_time_field() {
            let bridge = stub_bridge(vec!["return-data-200"]);
            let state = bridge.exchange_data(&ControlInputs::default()).unwrap();

            assert_relative_eq!(state.current_physics_time.get::<second>(), 72263.411813672);
        }
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
