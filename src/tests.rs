use rand::Rng;

#[cfg(feature = "uom")]
use uom::si::acceleration::meter_per_second_squared;
#[cfg(feature = "uom")]
use uom::si::angular_velocity::degree_per_second;
#[cfg(feature = "uom")]
use uom::si::electric_charge::milliampere_hour;
#[cfg(feature = "uom")]
use uom::si::electric_current::ampere;
#[cfg(feature = "uom")]
use uom::si::electric_potential::volt;
#[cfg(feature = "uom")]
use uom::si::length::meter;
#[cfg(feature = "uom")]
use uom::si::time::second;
#[cfg(feature = "uom")]
use uom::si::velocity::meter_per_second;
#[cfg(feature = "uom")]
use uom::si::volume::liter;

use crate::BridgeError;
use crate::bridge::local::{Configuration, RealFlightLocalBridge};
#[cfg(feature = "uom")]
use crate::decoders::OUNCES_PER_LITER;
use crate::soap_client::stub::StubSoapClient;

use approx::assert_relative_eq;

use super::*;

use soap_stub::Server;

fn create_configuration(port: u16) -> Configuration {
    Configuration {
        simulator_host: format!("127.0.0.1:{}", port),
        connect_timeout: Duration::from_millis(1000),
        pool_size: 1,
        ..Default::default()
    }
}

fn create_bridge(port: u16) -> Result<RealFlightLocalBridge, Box<dyn std::error::Error>> {
    let configuration = create_configuration(port);
    RealFlightLocalBridge::with_configuration(&configuration)
}

/// Generate a random port number. Mitigates chances of port conflicts.
fn random_port() -> u16 {
    let mut rng = rand::rng();
    10_000 + rng.random_range(1..1000)
}

#[test]
pub fn test_tcp_soap_client() {
    // Assemble
    let port = random_port();
    let server = Server::new(port, vec!["reset-aircraft-200".to_string()]);
    let bridge = create_bridge(port).unwrap();

    // Act
    let result = bridge.reset_aircraft();
    match result {
        Ok(_) => {}
        Err(e) => {
            panic!("expected Ok from bridge.reset_aircraft: {:?}", e);
        }
    }

    // Assert
    let statistics = bridge.statistics();
    assert_eq!(statistics.request_count, 1);
    assert_eq!(statistics.error_count, 0);

    drop(server);
}

#[test]
pub fn test_reset_aircraft() {
    // Assemble
    let soap_client = StubSoapClient::new(vec!["reset-aircraft-200".to_string()]);
    let bridge = RealFlightLocalBridge::stub(soap_client).unwrap();

    // Act
    let result = bridge.reset_aircraft();
    if let Err(ref e) = result {
        panic!("expected Ok from bridge.reset_aircraft: {:?}", e);
    }

    // Assert
    let requests = bridge.requests();

    assert_eq!(requests.len(), 1);
    let reset_request = "\
    <?xml version='1.0' encoding='UTF-8'?><soap:Envelope xmlns:soap='http://schemas.xmlsoap.org/soap/envelope/' xmlns:xsd='http://www.w3.org/2001/XMLSchema' xmlns:xsi='http://www.w3.org/2001/XMLSchema-instance'><soap:Body><ResetAircraft></ResetAircraft></soap:Body></soap:Envelope>\
    ";
    assert_eq!(requests[0], reset_request);

    let statistics = bridge.statistics();
    assert_eq!(statistics.request_count, 1);
}

#[test]
pub fn test_disable_rc_200() {
    // Assemble
    let soap_client = StubSoapClient::new(vec!["inject-uav-controller-interface-200".to_string()]);
    let bridge = RealFlightLocalBridge::stub(soap_client).unwrap();

    // Act
    let result = bridge.disable_rc();

    // Assert
    if let Err(ref e) = result {
        panic!("expected Ok from bridge.disable_rc: {:?}", e);
    }

    let requests = bridge.requests();
    assert_eq!(requests.len(), 1);
    let disable_request = "\
    <?xml version='1.0' encoding='UTF-8'?><soap:Envelope xmlns:soap='http://schemas.xmlsoap.org/soap/envelope/' xmlns:xsd='http://www.w3.org/2001/XMLSchema' xmlns:xsi='http://www.w3.org/2001/XMLSchema-instance'><soap:Body><InjectUAVControllerInterface></InjectUAVControllerInterface></soap:Body></soap:Envelope>\
    ";
    assert_eq!(requests[0], disable_request);

    let statistics = bridge.statistics();
    assert_eq!(statistics.request_count, 1);
}

#[test]
pub fn test_disable_rc_500() {
    // Assemble
    let soap_client = StubSoapClient::new(vec!["inject-uav-controller-interface-500".to_string()]);
    let bridge = RealFlightLocalBridge::stub(soap_client).unwrap();

    // Act
    let result = bridge.disable_rc();

    // Assert
    match result {
        Err(BridgeError::SoapFault(msg)) => {
            assert_eq!(msg, "Preexisting controller reference");
        }
        Err(e) => panic!("expected SoapFault, got {:?}", e),
        Ok(_) => panic!("expected error from bridge.disable_rc"),
    }
}

#[test]
pub fn test_enable_rc_200() {
    // Assemble
    let soap_client =
        StubSoapClient::new(vec!["restore-original-controller-device-200".to_string()]);
    let bridge = RealFlightLocalBridge::stub(soap_client).unwrap();

    // Act
    let result = bridge.enable_rc();

    // Assert
    if let Err(ref e) = result {
        panic!("expected Ok from bridge.enable_rc: {:?}", e);
    }

    let statistics = bridge.statistics();
    let requests = bridge.requests();

    let disable_request = "\
    <?xml version='1.0' encoding='UTF-8'?><soap:Envelope xmlns:soap='http://schemas.xmlsoap.org/soap/envelope/' xmlns:xsd='http://www.w3.org/2001/XMLSchema' xmlns:xsi='http://www.w3.org/2001/XMLSchema-instance'><soap:Body><RestoreOriginalControllerDevice></RestoreOriginalControllerDevice></soap:Body></soap:Envelope>\
    ";
    assert_eq!(requests[0], disable_request);
    assert_eq!(statistics.request_count, 1);
}

#[test]
pub fn test_enable_rc_500() {
    // Assemble
    let soap_client =
        StubSoapClient::new(vec!["restore-original-controller-device-500".to_string()]);
    let bridge = RealFlightLocalBridge::stub(soap_client).unwrap();

    // Act
    let result = bridge.enable_rc();

    // Assert
    match result {
        Err(BridgeError::SoapFault(msg)) => {
            assert_eq!(msg, "Pointer to original controller device is null");
        }
        Err(e) => panic!("expected SoapFault, got {:?}", e),
        Ok(_) => panic!("expected error from bridge.enable_rc"),
    }
}

#[test]
pub fn test_exchange_data_200() {
    // Test the exchange_data method with a successful response from the simulator.

    // Assemble
    let soap_client = StubSoapClient::new(vec!["return-data-200".to_string()]);
    let bridge = RealFlightLocalBridge::stub(soap_client).unwrap();

    // Act
    let mut control = ControlInputs::default();
    for i in 0..control.channels.len() {
        control.channels[i] = i as f32 / 12.0;
    }

    let result = bridge.exchange_data(&control);

    // Assert
    if let Err(ref e) = result {
        panic!("expected Ok from bridge.exchange_data: {:?}", e);
    }

    let state = result.unwrap();

    assert_eq!(state.current_physics_speed_multiplier, 1.0);

    #[cfg(feature = "uom")]
    {
        assert_relative_eq!(state.current_physics_time.get::<second>(), 72263.411813672);
        assert_relative_eq!(state.airspeed.get::<meter_per_second>(), 0.040872246);
        assert_relative_eq!(state.altitude_asl.get::<meter>(), 1127.370971679);
        assert_relative_eq!(state.altitude_agl.get::<meter>(), 0.266309916);
        assert_relative_eq!(state.groundspeed.get::<meter_per_second>(), 4.643444754E-06);
        assert_relative_eq!(state.pitch_rate.get::<degree_per_second>(), 0.001380353);
        assert_relative_eq!(state.roll_rate.get::<degree_per_second>(), -0.000032227);
        assert_relative_eq!(state.yaw_rate.get::<degree_per_second>(), 0.001473751);
        assert_relative_eq!(state.aircraft_position_x.get::<meter>(), 5575.6806640625);
        assert_relative_eq!(state.aircraft_position_y.get::<meter>(), 1715.962158203125);
        assert_relative_eq!(
            state.velocity_world_u.get::<meter_per_second>(),
            -2.005582700E-06
        );
        assert_relative_eq!(
            state.velocity_world_v.get::<meter_per_second>(),
            4.187984814E-06
        );
        assert_relative_eq!(
            state.velocity_world_w.get::<meter_per_second>(),
            0.040872246
        );
        assert_relative_eq!(
            state.velocity_body_u.get::<meter_per_second>(),
            -0.001089469
        );
        assert_relative_eq!(
            state.velocity_body_v.get::<meter_per_second>(),
            -0.000530726
        );
        assert_relative_eq!(state.velocity_body_w.get::<meter_per_second>(), 0.040854275);
        assert_relative_eq!(
            state
                .acceleration_world_ax
                .get::<meter_per_second_squared>(),
            -0.000483050
        );
        assert_relative_eq!(
            state
                .acceleration_world_ay
                .get::<meter_per_second_squared>(),
            0.001008689
        );
        assert_relative_eq!(
            state
                .acceleration_world_az
                .get::<meter_per_second_squared>(),
            9.844209671
        );
        assert_relative_eq!(
            state.acceleration_body_ax.get::<meter_per_second_squared>(),
            -0.000176936
        );
        assert_relative_eq!(
            state.acceleration_body_ay.get::<meter_per_second_squared>(),
            -8.662045001E-05
        );
        assert_relative_eq!(
            state.acceleration_body_az.get::<meter_per_second_squared>(),
            0.044223785
        );
        assert_relative_eq!(state.wind_x.get::<meter_per_second>(), 0.0);
        assert_relative_eq!(state.wind_y.get::<meter_per_second>(), 0.0);
        assert_relative_eq!(state.wind_z.get::<meter_per_second>(), 0.0);
    }

    #[cfg(not(feature = "uom"))]
    {
        assert_relative_eq!(state.current_physics_time, 72263.411813672);
        assert_relative_eq!(state.airspeed, 0.040872246);
        assert_relative_eq!(state.altitude_asl, 1127.370971679);
        assert_relative_eq!(state.altitude_agl, 0.266309916);
        assert_relative_eq!(state.groundspeed, 4.643444754E-06);
        assert_relative_eq!(state.pitch_rate, 0.001380353);
        assert_relative_eq!(state.roll_rate, -0.000032227);
        assert_relative_eq!(state.yaw_rate, 0.001473751);
        assert_relative_eq!(state.aircraft_position_x, 5575.6806640625);
        assert_relative_eq!(state.aircraft_position_y, 1715.962158203125);
        assert_relative_eq!(state.velocity_world_u, -2.005582700E-06);
        assert_relative_eq!(state.velocity_world_v, 4.187984814E-06);
        assert_relative_eq!(state.velocity_world_w, 0.040872246);
        assert_relative_eq!(state.velocity_body_u, -0.001089469);
        assert_relative_eq!(state.velocity_body_v, -0.000530726);
        assert_relative_eq!(state.velocity_body_w, 0.040854275);
        assert_relative_eq!(state.acceleration_world_ax, -0.000483050);
        assert_relative_eq!(state.acceleration_world_ay, 0.001008689);
        assert_relative_eq!(state.acceleration_world_az, 9.844209671);
        assert_relative_eq!(state.acceleration_body_ax, -0.000176936);
        assert_relative_eq!(state.acceleration_body_ay, -8.662045001E-05);
        assert_relative_eq!(state.acceleration_body_az, 0.044223785);
        assert_relative_eq!(state.wind_x, 0.0);
        assert_relative_eq!(state.wind_y, 0.0);
        assert_relative_eq!(state.wind_z, 0.0);
    }

    assert_relative_eq!(state.prop_rpm, 47.404716491);
    assert_relative_eq!(state.heli_main_rotor_rpm, -1.0);

    #[cfg(feature = "uom")]
    {
        assert_relative_eq!(state.battery_voltage.get::<volt>(), 12.599982261);
        assert_relative_eq!(state.battery_current_draw.get::<ampere>(), 0.0);
        assert_relative_eq!(
            state.battery_remaining_capacity.get::<milliampere_hour>(),
            3999.990722656
        );
        assert_relative_eq!(state.fuel_remaining.get::<liter>(), -1.0 / OUNCES_PER_LITER);
    }

    #[cfg(not(feature = "uom"))]
    {
        assert_relative_eq!(state.battery_voltage, 12.599982261);
        assert_relative_eq!(state.battery_current_draw, 0.0);
        assert_relative_eq!(state.battery_remaining_capacity, 3999.990722656);
        assert_relative_eq!(state.fuel_remaining, -1.0);
    }

    assert_eq!(state.is_locked, false);
    assert_eq!(state.has_lost_components, false);
    assert_eq!(state.an_engine_is_running, true);
    assert_eq!(state.is_touching_ground, false);
    assert_eq!(state.flight_axis_controller_is_active, true);
    assert_eq!(state.current_aircraft_status, "CAS-WAITINGTOLAUNCH");

    let requests = bridge.requests();

    assert_eq!(requests.len(), 1);
    let control_inputs = "\
    <?xml version='1.0' encoding='UTF-8'?><soap:Envelope xmlns:soap='http://schemas.xmlsoap.org/soap/envelope/' xmlns:xsd='http://www.w3.org/2001/XMLSchema' xmlns:xsi='http://www.w3.org/2001/XMLSchema-instance'><soap:Body><ExchangeData><pControlInputs><m-selectedChannels>4095</m-selectedChannels><m-channelValues-0to1><item>0</item><item>0.083333336</item><item>0.16666667</item><item>0.25</item><item>0.33333334</item><item>0.41666666</item><item>0.5</item><item>0.5833333</item><item>0.6666667</item><item>0.75</item><item>0.8333333</item><item>0.9166667</item></m-channelValues-0to1></pControlInputs></ExchangeData></soap:Body></soap:Envelope>\
    ";
    assert_eq!(requests[0], control_inputs);

    let statistics = bridge.statistics();

    assert_eq!(statistics.request_count, 1);
    //    assert_eq!(statistics.error_count, 0);
}

#[test]
pub fn test_exchange_data_500() {
    // Test the exchange_data method with a failed response from the simulator.

    // Assemble
    let soap_client = StubSoapClient::new(vec!["return-data-500".to_string()]);
    let bridge = RealFlightLocalBridge::stub(soap_client).unwrap();

    // Act
    let mut control = ControlInputs::default();
    for i in 0..control.channels.len() {
        control.channels[i] = i as f32 / 12.0;
    }

    let result = bridge.exchange_data(&control);

    // Assert
    match result {
        Err(BridgeError::SoapFault(msg)) => {
            assert_eq!(msg, "RealFlight Link controller has not been instantiated");
        }
        Err(e) => panic!("expected SoapFault, got {:?}", e),
        Ok(_) => panic!("expected error from bridge.exchange_data"),
    }

    let requests = bridge.requests();

    assert_eq!(requests.len(), 1);
    let control_inputs = "\
    <?xml version='1.0' encoding='UTF-8'?><soap:Envelope xmlns:soap='http://schemas.xmlsoap.org/soap/envelope/' xmlns:xsd='http://www.w3.org/2001/XMLSchema' xmlns:xsi='http://www.w3.org/2001/XMLSchema-instance'><soap:Body><ExchangeData><pControlInputs><m-selectedChannels>4095</m-selectedChannels><m-channelValues-0to1><item>0</item><item>0.083333336</item><item>0.16666667</item><item>0.25</item><item>0.33333334</item><item>0.41666666</item><item>0.5</item><item>0.5833333</item><item>0.6666667</item><item>0.75</item><item>0.8333333</item><item>0.9166667</item></m-channelValues-0to1></pControlInputs></ExchangeData></soap:Body></soap:Envelope>\
    ";
    assert_eq!(requests[0], control_inputs);

    let statistics = bridge.statistics();
    assert_eq!(statistics.request_count, 1);
}

#[cfg(test)]
pub mod soap_stub;
