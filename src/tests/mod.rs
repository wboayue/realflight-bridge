use rand::Rng;

use serial_test::serial;
use uom::si::angular_velocity::degree_per_second;
use uom::si::f64::*;
use uom::si::length::kilometer;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;
use uom::si::angle::degree;

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
#[serial]
pub fn test_reset_aircraft() {
    let port: u16 = random_port();

    let server = Server::new(port, vec!["reset-aircraft-200".to_string()]);

    let bridge = create_bridge(port);

    let result = bridge.reset_aircraft();
    if let Err(ref e) = result {
        panic!("expected Ok from bridge.reset_aircraft: {:?}", e);
    }

    assert_eq!(server.request_count(), 1);

    let requests = server.requests();

    let reset_request = "\
    <?xml version='1.0' encoding='UTF-8'?><soap:Envelope xmlns:soap='http://schemas.xmlsoap.org/soap/envelope/' xmlns:xsd='http://www.w3.org/2001/XMLSchema' xmlns:xsi='http://www.w3.org/2001/XMLSchema-instance'><soap:Body><ResetAircraft></ResetAircraft></soap:Body></soap:Envelope>\
    ";

    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0], reset_request);

    let statistics = bridge.statistics();

    assert_eq!(statistics.request_count, 1);
}

#[test]
#[serial]
pub fn test_disable_rc_200() {
    let port: u16 = random_port();

    let server = Server::new(
        port,
        vec!["inject-uav-controller-interface-200".to_string()],
    );

    let bridge = create_bridge(port);

    let result = bridge.disable_rc();
    if let Err(ref e) = result {
        panic!("expected Ok from bridge.disable_rc: {:?}", e);
    }

    let requests = server.requests();

    assert_eq!(server.request_count(), 1);
    let disable_request = "\
    <?xml version='1.0' encoding='UTF-8'?><soap:Envelope xmlns:soap='http://schemas.xmlsoap.org/soap/envelope/' xmlns:xsd='http://www.w3.org/2001/XMLSchema' xmlns:xsi='http://www.w3.org/2001/XMLSchema-instance'><soap:Body><InjectUAVControllerInterface></InjectUAVControllerInterface></soap:Body></soap:Envelope>\
    ";
    assert_eq!(requests[0], disable_request);

    let statistics = bridge.statistics();

    assert_eq!(statistics.request_count, 1);
}

#[test]
#[serial]
pub fn test_disable_rc_500() {
    let port: u16 = random_port();

    let server = Server::new(
        port,
        vec!["inject-uav-controller-interface-500".to_string()],
    );

    let bridge = create_bridge(port);

    let result = bridge.disable_rc();
    match result {
        Err(e) => {
            assert_eq!(e.to_string(), "Preexisting controller reference");
        }
        _ => panic!("expected error from bridge.disable_rc"),
    }

    drop(server);
}

#[test]
#[serial]
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
    if let Err(ref e) = result {
        panic!("expected Ok from bridge.enable_rc: {:?}", e);
    }

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
#[serial]
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
    if let Err(ref e) = result {
        panic!("expected Ok from bridge.exchange_data: {:?}", e);
    }

    let state = result.unwrap();

    assert_eq!(state.current_physics_time, Time::new::<second>(72263.411813672516));
    assert_eq!(state.current_physics_speed_multiplier, 1.0);
    assert_eq!(state.airspeed, Velocity::new::<meter_per_second>(0.146543));
    assert_eq!(state.altitude_asl, Length::new::<kilometer>(1.1273709716796875));
    assert_eq!(state.altitude_agl, Length::new::<kilometer>(0.000266309916973114));
    assert_eq!(state.groundspeed, Velocity::new::<meter_per_second>(0.0000046434447540377732));
    assert_eq!(state.pitch_rate, AngularVelocity::new::<degree_per_second>(0.0013803535839542747));
    assert_eq!(state.roll_rate, AngularVelocity::new::<degree_per_second>(-0.00003222789746359922));
    assert_eq!(state.yaw_rate, AngularVelocity::new::<degree_per_second>(0.0014737510355189443));
    assert_eq!(state.azimuth, Angle::new::<degree>(-89.6070556640625));
    assert_eq!(state.inclination, Angle::new::<degree>(1.533278226852417));
    assert_eq!(state.roll, Angle::new::<degree>(-0.74712425470352173));
    assert_eq!(state.orientation_quaternion_x, 0.0048992796801030636);
    assert_eq!(state.orientation_quaternion_y, -0.014053969644010067);
    assert_eq!(state.orientation_quaternion_z, -0.7046617865562439);
    assert_eq!(state.orientation_quaternion_w, 0.70938730239868164);
    // <m-airspeed-MPS>0.040872246026992798</m-airspeed-MPS>
    // <m-altitudeASL-MTR>1127.3709716796875</m-altitudeASL-MTR>
    // <m-altitudeAGL-MTR>0.26630991697311401</m-altitudeAGL-MTR>
    // <m-groundspeed-MPS>4.6434447540377732E-06</m-groundspeed-MPS>
    // <m-pitchRate-DEGpSEC>0.0013803535839542747</m-pitchRate-DEGpSEC>
    // <m-rollRate-DEGpSEC>-3.222789746359922E-05</m-rollRate-DEGpSEC>
    // <m-yawRate-DEGpSEC>0.0014737510355189443</m-yawRate-DEGpSEC>
    // <m-azimuth-DEG>-89.6070556640625</m-azimuth-DEG>
    // <m-inclination-DEG>1.533278226852417</m-inclination-DEG>
    // <m-roll-DEG>-0.74712425470352173</m-roll-DEG>
    // <m-orientationQuaternion-X>0.0048992796801030636</m-orientationQuaternion-X>
    // <m-orientationQuaternion-Y>-0.014053969644010067</m-orientationQuaternion-Y>
    // <m-orientationQuaternion-Z>-0.7046617865562439</m-orientationQuaternion-Z>
    // <m-orientationQuaternion-W>0.70938730239868164</m-orientationQuaternion-W>

    let requests = server.requests();

    assert_eq!(server.request_count(), 1);
    let control_inputs = "\
    <?xml version='1.0' encoding='UTF-8'?><soap:Envelope xmlns:soap='http://schemas.xmlsoap.org/soap/envelope/' xmlns:xsd='http://www.w3.org/2001/XMLSchema' xmlns:xsi='http://www.w3.org/2001/XMLSchema-instance'><soap:Body><ExchangeData><pControlInputs><m-selectedChannels>4095</m-selectedChannels><m-channelValues-0to1><item>0</item><item>0.083333336</item><item>0.16666667</item><item>0.25</item><item>0.33333334</item><item>0.41666666</item><item>0.5</item><item>0.5833333</item><item>0.6666667</item><item>0.75</item><item>0.8333333</item><item>0.9166667</item></m-channelValues-0to1></pControlInputs></ExchangeData></soap:Body></soap:Envelope>\
    ";
    assert_eq!(requests[0], control_inputs);

    let statistics = bridge.statistics();

    assert_eq!(statistics.request_count, 1);
    //    assert_eq!(statistics.error_count, 0);
}

#[test]
#[serial]
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
            e.to_string(),
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
}

#[cfg(test)]
pub mod soap_stub;
