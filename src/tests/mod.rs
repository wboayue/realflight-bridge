use rand::Rng;

use uom::si::acceleration::meter_per_second_squared;
use uom::si::angle::degree;
use uom::si::angular_velocity::degree_per_second;
use uom::si::electric_charge::milliampere_hour;
use uom::si::electric_current::ampere;
use uom::si::electric_potential::volt;
use uom::si::f32::*;
use uom::si::length::meter;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;
use uom::si::volume::liter;

use super::*;
use crate::decoders::OUNCES_PER_LITER;
use soap_stub::Server;

fn create_configuration(port: u16) -> Configuration {
    Configuration {
        simulator_host: format!("127.0.0.1:{}", port),
        connect_timeout: Duration::from_millis(1000),
        pool_size: 1,
        ..Default::default()
    }
}

fn create_bridge(port: u16) -> Result<RealFlightBridge, Box<dyn std::error::Error>> {
    let configuration = create_configuration(port);
    RealFlightBridge::with_configuration(&configuration)
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
    let bridge = RealFlightBridge::stub(soap_client).unwrap();

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
    let bridge = RealFlightBridge::stub(soap_client).unwrap();

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
    let bridge = RealFlightBridge::stub(soap_client).unwrap();

    // Act
    let result = bridge.disable_rc();

    // Assert
    match result {
        Err(e) => {
            assert_eq!(e.to_string(), "Preexisting controller reference");
        }
        _ => panic!("expected error from bridge.disable_rc"),
    }
}

#[test]
pub fn test_enable_rc_200() {
    // Assemble
    let soap_client =
        StubSoapClient::new(vec!["restore-original-controller-device-200".to_string()]);
    let bridge = RealFlightBridge::stub(soap_client).unwrap();

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
    let bridge = RealFlightBridge::stub(soap_client).unwrap();

    // Act
    let result = bridge.enable_rc();

    // Assert
    match result {
        Err(e) => {
            assert_eq!(
                e.to_string(),
                "Pointer to original controller device is null"
            );
        }
        _ => panic!("expected error from bridge.enable_rc"),
    }
}

#[test]
pub fn test_exchange_data_200() {
    // Test the exchange_data method with a successful response from the simulator.

    // Assemble
    let soap_client = StubSoapClient::new(vec!["return-data-200".to_string()]);
    let bridge = RealFlightBridge::stub(soap_client).unwrap();

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

    assert_eq!(
        state.current_physics_time,
        Time::new::<second>(72263.411813672516)
    );
    assert_eq!(state.current_physics_speed_multiplier, 1.0);
    assert_eq!(
        state.airspeed,
        Velocity::new::<meter_per_second>(0.040872246026992798)
    );
    assert_eq!(state.altitude_asl, Length::new::<meter>(1127.3709716796875));
    assert_eq!(
        state.altitude_agl,
        Length::new::<meter>(0.26630991697311401)
    );
    assert_eq!(
        state.groundspeed,
        Velocity::new::<meter_per_second>(4.6434447540377732E-06)
    );
    assert_eq!(
        state.pitch_rate,
        AngularVelocity::new::<degree_per_second>(0.0013803535839542747)
    );
    assert_eq!(
        state.roll_rate,
        AngularVelocity::new::<degree_per_second>(-0.00003222789746359922)
    );
    assert_eq!(
        state.yaw_rate,
        AngularVelocity::new::<degree_per_second>(0.0014737510355189443)
    );
    assert_eq!(state.azimuth, Angle::new::<degree>(-89.6070556640625));
    assert_eq!(state.inclination, Angle::new::<degree>(1.533278226852417));
    assert_eq!(state.roll, Angle::new::<degree>(-0.74712425470352173));
    assert_eq!(state.orientation_quaternion_x, 0.0048992796801030636);
    assert_eq!(state.orientation_quaternion_y, -0.014053969644010067);
    assert_eq!(state.orientation_quaternion_z, -0.7046617865562439);
    assert_eq!(state.orientation_quaternion_w, 0.70938730239868164);
    assert_eq!(
        state.aircraft_position_x,
        Length::new::<meter>(5575.6806640625)
    );
    assert_eq!(
        state.aircraft_position_y,
        Length::new::<meter>(1715.962158203125)
    );
    assert_eq!(
        state.velocity_world_u,
        Velocity::new::<meter_per_second>(-2.0055827008036431E-06)
    );
    assert_eq!(
        state.velocity_world_v,
        Velocity::new::<meter_per_second>(4.18798481405247E-06)
    );
    assert_eq!(
        state.velocity_world_w,
        Velocity::new::<meter_per_second>(0.040872246026992798)
    );
    assert_eq!(
        state.velocity_body_u,
        Velocity::new::<meter_per_second>(-0.001089469064027071)
    );
    assert_eq!(
        state.velocity_body_v,
        Velocity::new::<meter_per_second>(-0.0005307266837917268)
    );
    assert_eq!(
        state.velocity_body_w,
        Velocity::new::<meter_per_second>(0.04085427522659302)
    );
    assert_eq!(
        state.acceleration_world_ax,
        Acceleration::new::<meter_per_second_squared>(-0.00048305094242095947)
    );
    assert_eq!(
        state.acceleration_world_ay,
        Acceleration::new::<meter_per_second_squared>(0.0010086894035339355)
    );
    assert_eq!(
        state.acceleration_world_az,
        Acceleration::new::<meter_per_second_squared>(9.844209671020508)
    );
    assert_eq!(
        state.acceleration_body_ax,
        Acceleration::new::<meter_per_second_squared>(-0.00017693638801574707)
    );
    assert_eq!(
        state.acceleration_body_ay,
        Acceleration::new::<meter_per_second_squared>(-8.6620450019836426E-05)
    );
    assert_eq!(
        state.acceleration_body_az,
        Acceleration::new::<meter_per_second_squared>(0.044223785400390625)
    );
    assert_eq!(state.wind_x, Velocity::new::<meter_per_second>(0.0));
    assert_eq!(state.wind_y, Velocity::new::<meter_per_second>(0.0));
    assert_eq!(state.wind_z, Velocity::new::<meter_per_second>(0.0));
    assert_eq!(state.prop_rpm, 47.40471649169922);
    assert_eq!(state.heli_main_rotor_rpm, -1.0);
    assert_eq!(
        state.battery_voltage,
        ElectricPotential::new::<volt>(12.599982261657715)
    );
    assert_eq!(
        state.battery_current_draw,
        ElectricCurrent::new::<ampere>(0.0)
    );
    assert_eq!(
        state.battery_remaining_capacity,
        ElectricCharge::new::<milliampere_hour>(3999.99072265625)
    );
    assert_eq!(
        state.fuel_remaining,
        Volume::new::<liter>(-1.0 / OUNCES_PER_LITER)
    );
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
    let bridge = RealFlightBridge::stub(soap_client).unwrap();

    // Act
    let mut control = ControlInputs::default();
    for i in 0..control.channels.len() {
        control.channels[i] = i as f32 / 12.0;
    }

    let result = bridge.exchange_data(&control);

    // Assert
    if let Err(ref e) = result {
        assert_eq!(
            e.to_string(),
            "RealFlight Link controller has not been instantiated"
        );
    } else {
        panic!("expected error from bridge.exchange_data");
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
