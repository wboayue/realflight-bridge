use approx::assert_relative_eq;

use super::*;

static SIM_STATE_RESPONSE: &str = include_str!("../../testdata/responses/return-data-200.xml");

#[cfg(not(feature = "uom"))]
#[test]
pub fn test_decode_simulator_state() {
    // Tests the decoding of simulator state.

    // Act
    let state =
        decode_simulator_state(SIM_STATE_RESPONSE).expect("Failed to decode simulator state");

    // Assert
    assert_eq!(
        state.previous_inputs.channels,
        [0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.0]
    );

    assert_relative_eq!(state.current_physics_time, 72263.411813672);
    assert_relative_eq!(state.current_physics_speed_multiplier, 1.0);
    assert_relative_eq!(state.airspeed, 0.040872246);
    assert_relative_eq!(state.altitude_asl, 1127.370971679);
    assert_relative_eq!(state.altitude_agl, 0.266309916);
    assert_relative_eq!(state.groundspeed, 4.6434447540377732E-06);
    assert_relative_eq!(state.pitch_rate, 0.001380353);
    assert_relative_eq!(state.roll_rate, -0.000032227);
    assert_relative_eq!(state.yaw_rate, 0.001473751);
    assert_relative_eq!(state.azimuth, -89.607055664);
    assert_relative_eq!(state.inclination, 1.533278226);
    assert_relative_eq!(state.roll, -0.747124254);
    assert_relative_eq!(state.orientation_quaternion_x, 0.004899279);
    assert_relative_eq!(state.orientation_quaternion_y, -0.014053969);
    assert_relative_eq!(state.orientation_quaternion_z, -0.704661786);
    assert_relative_eq!(state.orientation_quaternion_w, 0.709387302);
    assert_relative_eq!(state.aircraft_position_x, 5575.680664062);
    assert_relative_eq!(state.aircraft_position_y, 1715.962158203);
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
    assert_relative_eq!(state.acceleration_body_ay, -0.000086620);
    assert_relative_eq!(state.acceleration_body_az, 0.044223785);
    assert_relative_eq!(state.wind_x, 0.0);
    assert_relative_eq!(state.wind_y, 0.0);
    assert_relative_eq!(state.wind_z, 0.0);
    assert_relative_eq!(state.prop_rpm, 47.404716491);
    assert_relative_eq!(state.heli_main_rotor_rpm, -1.0);
    assert_relative_eq!(state.battery_voltage, 12.599982261);
    assert_relative_eq!(state.battery_current_draw, 0.0);
    assert_relative_eq!(state.battery_remaining_capacity, 3999.990722656);
    assert_relative_eq!(state.fuel_remaining, -1.0);
    assert_eq!(state.is_locked, false);
    assert_eq!(state.has_lost_components, false);
    assert_eq!(state.an_engine_is_running, true);
    assert_eq!(state.is_touching_ground, false);
    assert_eq!(state.flight_axis_controller_is_active, true);
    assert_eq!(state.current_aircraft_status, "CAS-WAITINGTOLAUNCH");
}

#[cfg(feature = "uom")]
#[test]
pub fn test_decode_simulator_state() {
    // Tests the decoding of simulator state.

    // Act
    let state =
        decode_simulator_state(SIM_STATE_RESPONSE).expect("Failed to decode simulator state");

    // Assert
    assert_eq!(
        state.previous_inputs.channels,
        [0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.0]
    );

    assert_eq!(state.current_physics_time.get::<second>(), 72263.411813672);
    assert_eq!(state.current_physics_speed_multiplier, 1.0);
    assert_eq!(state.airspeed.get::<meter_per_second>(), 0.040872246);
    assert_eq!(state.altitude_asl, Length::new::<meter>(1127.370971679));
    assert_eq!(state.altitude_agl, Length::new::<meter>(0.266309916));
    assert_eq!(state.groundspeed.get::<meter_per_second>(), 4.643444754E-06);
    assert_relative_eq!(state.pitch_rate.get::<degree_per_second>(), 0.001380353);
    assert_relative_eq!(state.roll_rate.get::<degree_per_second>(), -0.000032227);
    assert_relative_eq!(state.yaw_rate.get::<degree_per_second>(), 0.001473751);
    assert_eq!(state.azimuth, Angle::new::<degree>(-89.607055664));
    assert_eq!(state.inclination, Angle::new::<degree>(1.533278226));
    assert_eq!(state.roll, Angle::new::<degree>(-0.74712425470352173));
    assert_relative_eq!(state.orientation_quaternion_x, 0.004899279);
    assert_relative_eq!(state.orientation_quaternion_y, -0.014053969);
    assert_relative_eq!(state.orientation_quaternion_z, -0.704661786);
    assert_relative_eq!(state.orientation_quaternion_w, 0.709387302);
    assert_relative_eq!(state.aircraft_position_x.get::<meter>(), 5575.680664062);
    assert_relative_eq!(state.aircraft_position_y.get::<meter>(), 1715.962158203);
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
    assert_relative_eq!(state.prop_rpm, 47.404716491);
    assert_relative_eq!(state.heli_main_rotor_rpm, -1.0);
    assert_relative_eq!(state.battery_voltage.get::<volt>(), 12.599982261);
    assert_relative_eq!(state.battery_current_draw.get::<ampere>(), 0.0);
    assert_relative_eq!(
        state.battery_remaining_capacity.get::<milliampere_hour>(),
        3999.990722656
    );
    assert_relative_eq!(state.fuel_remaining.get::<liter>(), -1.0 / OUNCES_PER_LITER);
    assert_eq!(state.is_locked, false);
    assert_eq!(state.has_lost_components, false);
    assert_eq!(state.an_engine_is_running, true);
    assert_eq!(state.is_touching_ground, false);
    assert_eq!(state.flight_axis_controller_is_active, true);
    assert_eq!(state.current_aircraft_status, "CAS-WAITINGTOLAUNCH");
}
