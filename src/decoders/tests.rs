use super::*;

static SIM_STATE_RESPONSE: &str = include_str!("../../testdata/responses/return-data-200.xml");

#[test]
pub fn test_decode_simulator_state() {
    // Tests the decoding of simulator state.

    // Act
    let state = decode_simulator_state(SIM_STATE_RESPONSE).expect("Failed to decode simulator state");

    // Assert
    assert_eq!(
        state.previous_inputs.channels,
        [0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.0]
    );

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
}
