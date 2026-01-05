//! Tests for the decoder module.
//!
//! Organized into submodules:
//! - `extract_element_tests`: Tests for XML element extraction
//! - `decode_state_tests`: Tests for full simulator state decoding
//! - `error_handling`: Tests for parse error handling

use approx::assert_relative_eq;

use super::*;

static SIM_STATE_RESPONSE: &str = include_str!("../../testdata/responses/return-data-200.xml");

// ============================================================================
// extract_element Tests
// ============================================================================

mod extract_element_tests {
    use super::*;

    #[test]
    fn extracts_simple_element() {
        let xml = "<root><name>value</name></root>";
        let result = extract_element("name", xml);
        assert_eq!(result, Some("value".to_string()));
    }

    #[test]
    fn extracts_nested_element() {
        let xml = "<root><outer><inner>nested</inner></outer></root>";
        let result = extract_element("inner", xml);
        assert_eq!(result, Some("nested".to_string()));
    }

    #[test]
    fn returns_none_for_missing_element() {
        let xml = "<root><name>value</name></root>";
        let result = extract_element("missing", xml);
        assert_eq!(result, None);
    }

    #[test]
    fn returns_none_for_empty_input() {
        let result = extract_element("name", "");
        assert_eq!(result, None);
    }

    #[test]
    fn returns_none_for_unclosed_tag() {
        let xml = "<name>value";
        let result = extract_element("name", xml);
        assert_eq!(result, None);
    }

    #[test]
    fn returns_none_for_missing_close_tag() {
        let xml = "<name>value<other></other>";
        let result = extract_element("name", xml);
        assert_eq!(result, None);
    }

    #[test]
    fn extracts_empty_element_value() {
        let xml = "<root><empty></empty></root>";
        let result = extract_element("empty", xml);
        assert_eq!(result, None); // Empty content between tags
    }

    #[test]
    fn extracts_numeric_value() {
        let xml = "<data><value>123.456</value></data>";
        let result = extract_element("value", xml);
        assert_eq!(result, Some("123.456".to_string()));
    }

    #[test]
    fn extracts_first_occurrence() {
        let xml = "<root><item>first</item><item>second</item></root>";
        let result = extract_element("item", xml);
        assert_eq!(result, Some("first".to_string()));
    }

    #[test]
    fn handles_whitespace_in_value() {
        let xml = "<root><msg>  spaced  </msg></root>";
        let result = extract_element("msg", xml);
        assert_eq!(result, Some("  spaced  ".to_string()));
    }

    #[test]
    fn extracts_detail_from_soap_fault() {
        let xml = r#"<soap:Fault><faultcode>soap:Client</faultcode><detail>Error message</detail></soap:Fault>"#;
        let result = extract_element("detail", xml);
        assert_eq!(result, Some("Error message".to_string()));
    }
}

// ============================================================================
// decode_simulator_state Error Handling Tests
// ============================================================================

mod error_handling {
    use super::*;

    #[test]
    fn handles_empty_response() {
        let result = decode_simulator_state("");
        // Should succeed with default values
        assert!(result.is_ok());
    }

    #[test]
    fn handles_minimal_valid_xml() {
        let xml = "<?xml version='1.0'?><root></root>";
        let result = decode_simulator_state(xml);
        assert!(result.is_ok());
    }

    #[test]
    fn returns_error_for_invalid_numeric_value() {
        // Create XML with an invalid numeric field
        let xml = r#"<m-airspeed-MPS>not_a_number</m-airspeed-MPS>"#;
        let result = decode_simulator_state(xml);

        match result {
            Err(BridgeError::Parse { field, .. }) => {
                assert_eq!(field, "m-airspeed-MPS");
            }
            other => panic!("expected Parse error, got {:?}", other),
        }
    }

    #[test]
    fn returns_error_for_invalid_boolean() {
        let xml = r#"<m-isLocked>maybe</m-isLocked>"#;
        let result = decode_simulator_state(xml);

        match result {
            Err(BridgeError::Parse { field, .. }) => {
                assert_eq!(field, "m-isLocked");
            }
            other => panic!("expected Parse error, got {:?}", other),
        }
    }

    #[test]
    fn returns_error_for_invalid_channel_value() {
        let xml = r#"<m-channelValues-0to1><item>not_float</item></m-channelValues-0to1>"#;
        let result = decode_simulator_state(xml);

        match result {
            Err(BridgeError::Parse { field, .. }) => {
                assert!(field.contains("channel"));
            }
            other => panic!("expected Parse error, got {:?}", other),
        }
    }

    #[test]
    fn ignores_unknown_fields() {
        let xml = r#"<unknown-field>some value</unknown-field><m-propRPM>100.0</m-propRPM>"#;
        let result = decode_simulator_state(xml);

        assert!(result.is_ok());
        let state = result.unwrap();
        assert_relative_eq!(state.prop_rpm, 100.0);
    }
}

// ============================================================================
// decode_simulator_state Full Parsing Tests (without uom)
// ============================================================================

#[cfg(not(feature = "uom"))]
mod decode_state_raw_values {
    use super::*;

    #[test]
    fn parses_previous_channel_inputs() {
        let state =
            decode_simulator_state(SIM_STATE_RESPONSE).expect("Failed to decode simulator state");

        assert_eq!(
            state.previous_inputs.channels,
            [0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.0]
        );
    }

    #[test]
    fn parses_time_and_speed() {
        let state = decode_simulator_state(SIM_STATE_RESPONSE).unwrap();

        assert_relative_eq!(state.current_physics_time, 72263.411813672);
        assert_relative_eq!(state.current_physics_speed_multiplier, 1.0);
    }

    #[test]
    fn parses_velocity_fields() {
        let state = decode_simulator_state(SIM_STATE_RESPONSE).unwrap();

        assert_relative_eq!(state.airspeed, 0.040872246);
        assert_relative_eq!(state.groundspeed, 4.6434447540377732E-06);
        assert_relative_eq!(state.velocity_world_u, -2.005582700E-06);
        assert_relative_eq!(state.velocity_world_v, 4.187984814E-06);
        assert_relative_eq!(state.velocity_world_w, 0.040872246);
        assert_relative_eq!(state.velocity_body_u, -0.001089469);
        assert_relative_eq!(state.velocity_body_v, -0.000530726);
        assert_relative_eq!(state.velocity_body_w, 0.040854275);
    }

    #[test]
    fn parses_position_fields() {
        let state = decode_simulator_state(SIM_STATE_RESPONSE).unwrap();

        assert_relative_eq!(state.altitude_asl, 1127.370971679);
        assert_relative_eq!(state.altitude_agl, 0.266309916);
        assert_relative_eq!(state.aircraft_position_x, 5575.680664062);
        assert_relative_eq!(state.aircraft_position_y, 1715.962158203);
    }

    #[test]
    fn parses_angular_rate_fields() {
        let state = decode_simulator_state(SIM_STATE_RESPONSE).unwrap();

        assert_relative_eq!(state.pitch_rate, 0.001380353);
        assert_relative_eq!(state.roll_rate, -0.000032227);
        assert_relative_eq!(state.yaw_rate, 0.001473751);
    }

    #[test]
    fn parses_orientation_fields() {
        let state = decode_simulator_state(SIM_STATE_RESPONSE).unwrap();

        assert_relative_eq!(state.azimuth, -89.607055664);
        assert_relative_eq!(state.inclination, 1.533278226);
        assert_relative_eq!(state.roll, -0.747124254);
    }

    #[test]
    fn parses_quaternion_fields() {
        let state = decode_simulator_state(SIM_STATE_RESPONSE).unwrap();

        assert_relative_eq!(state.orientation_quaternion_x, 0.004899279);
        assert_relative_eq!(state.orientation_quaternion_y, -0.014053969);
        assert_relative_eq!(state.orientation_quaternion_z, -0.704661786);
        assert_relative_eq!(state.orientation_quaternion_w, 0.709387302);
    }

    #[test]
    fn parses_acceleration_fields() {
        let state = decode_simulator_state(SIM_STATE_RESPONSE).unwrap();

        assert_relative_eq!(state.acceleration_world_ax, -0.000483050);
        assert_relative_eq!(state.acceleration_world_ay, 0.001008689);
        assert_relative_eq!(state.acceleration_world_az, 9.844209671);
        assert_relative_eq!(state.acceleration_body_ax, -0.000176936);
        assert_relative_eq!(state.acceleration_body_ay, -0.000086620);
        assert_relative_eq!(state.acceleration_body_az, 0.044223785);
    }

    #[test]
    fn parses_wind_fields() {
        let state = decode_simulator_state(SIM_STATE_RESPONSE).unwrap();

        assert_relative_eq!(state.wind_x, 0.0);
        assert_relative_eq!(state.wind_y, 0.0);
        assert_relative_eq!(state.wind_z, 0.0);
    }

    #[test]
    fn parses_engine_fields() {
        let state = decode_simulator_state(SIM_STATE_RESPONSE).unwrap();

        assert_relative_eq!(state.prop_rpm, 47.404716491);
        assert_relative_eq!(state.heli_main_rotor_rpm, -1.0);
    }

    #[test]
    fn parses_battery_fields() {
        let state = decode_simulator_state(SIM_STATE_RESPONSE).unwrap();

        assert_relative_eq!(state.battery_voltage, 12.599982261);
        assert_relative_eq!(state.battery_current_draw, 0.0);
        assert_relative_eq!(state.battery_remaining_capacity, 3999.990722656);
        assert_relative_eq!(state.fuel_remaining, -1.0);
    }

    #[test]
    fn parses_boolean_fields() {
        let state = decode_simulator_state(SIM_STATE_RESPONSE).unwrap();

        assert!(!state.is_locked);
        assert!(!state.has_lost_components);
        assert!(state.an_engine_is_running);
        assert!(!state.is_touching_ground);
        assert!(state.flight_axis_controller_is_active);
    }

    #[test]
    fn parses_status_field() {
        let state = decode_simulator_state(SIM_STATE_RESPONSE).unwrap();
        assert_eq!(state.current_aircraft_status, "CAS-WAITINGTOLAUNCH");
    }
}

// ============================================================================
// decode_simulator_state Full Parsing Tests (with uom)
// ============================================================================

#[cfg(feature = "uom")]
mod decode_state_with_units {
    use super::*;
    use uom::si::acceleration::meter_per_second_squared;
    use uom::si::angle::degree;
    use uom::si::angular_velocity::degree_per_second;
    use uom::si::electric_charge::milliampere_hour;
    use uom::si::electric_current::ampere;
    use uom::si::electric_potential::volt;
    use uom::si::f32::{Angle, Length};
    use uom::si::length::meter;
    use uom::si::time::second;
    use uom::si::velocity::meter_per_second;
    use uom::si::volume::liter;

    #[test]
    fn parses_previous_channel_inputs() {
        let state =
            decode_simulator_state(SIM_STATE_RESPONSE).expect("Failed to decode simulator state");

        assert_eq!(
            state.previous_inputs.channels,
            [0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.0]
        );
    }

    #[test]
    fn parses_time_and_speed() {
        let state = decode_simulator_state(SIM_STATE_RESPONSE).unwrap();

        assert_eq!(state.current_physics_time.get::<second>(), 72263.411813672);
        assert_eq!(state.current_physics_speed_multiplier, 1.0);
    }

    #[test]
    fn parses_velocity_fields() {
        let state = decode_simulator_state(SIM_STATE_RESPONSE).unwrap();

        assert_eq!(state.airspeed.get::<meter_per_second>(), 0.040872246);
        assert_eq!(state.groundspeed.get::<meter_per_second>(), 4.643444754E-06);
    }

    #[test]
    fn parses_position_fields() {
        let state = decode_simulator_state(SIM_STATE_RESPONSE).unwrap();

        assert_eq!(state.altitude_asl, Length::new::<meter>(1127.370971679));
        assert_eq!(state.altitude_agl, Length::new::<meter>(0.266309916));
        assert_relative_eq!(state.aircraft_position_x.get::<meter>(), 5575.680664062);
        assert_relative_eq!(state.aircraft_position_y.get::<meter>(), 1715.962158203);
    }

    #[test]
    fn parses_angular_rate_fields() {
        let state = decode_simulator_state(SIM_STATE_RESPONSE).unwrap();

        assert_relative_eq!(state.pitch_rate.get::<degree_per_second>(), 0.001380353);
        assert_relative_eq!(state.roll_rate.get::<degree_per_second>(), -0.000032227);
        assert_relative_eq!(state.yaw_rate.get::<degree_per_second>(), 0.001473751);
    }

    #[test]
    fn parses_orientation_fields() {
        let state = decode_simulator_state(SIM_STATE_RESPONSE).unwrap();

        assert_eq!(state.azimuth, Angle::new::<degree>(-89.607055664));
        assert_eq!(state.inclination, Angle::new::<degree>(1.533278226));
        assert_eq!(state.roll, Angle::new::<degree>(-0.74712425470352173));
    }

    #[test]
    fn parses_quaternion_fields() {
        let state = decode_simulator_state(SIM_STATE_RESPONSE).unwrap();

        assert_relative_eq!(state.orientation_quaternion_x, 0.004899279);
        assert_relative_eq!(state.orientation_quaternion_y, -0.014053969);
        assert_relative_eq!(state.orientation_quaternion_z, -0.704661786);
        assert_relative_eq!(state.orientation_quaternion_w, 0.709387302);
    }

    #[test]
    fn parses_acceleration_fields() {
        let state = decode_simulator_state(SIM_STATE_RESPONSE).unwrap();

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
    }

    #[test]
    fn parses_wind_fields() {
        let state = decode_simulator_state(SIM_STATE_RESPONSE).unwrap();

        assert_relative_eq!(state.wind_x.get::<meter_per_second>(), 0.0);
        assert_relative_eq!(state.wind_y.get::<meter_per_second>(), 0.0);
        assert_relative_eq!(state.wind_z.get::<meter_per_second>(), 0.0);
    }

    #[test]
    fn parses_engine_fields() {
        let state = decode_simulator_state(SIM_STATE_RESPONSE).unwrap();

        assert_relative_eq!(state.prop_rpm, 47.404716491);
        assert_relative_eq!(state.heli_main_rotor_rpm, -1.0);
    }

    #[test]
    fn parses_battery_fields() {
        let state = decode_simulator_state(SIM_STATE_RESPONSE).unwrap();

        assert_relative_eq!(state.battery_voltage.get::<volt>(), 12.599982261);
        assert_relative_eq!(state.battery_current_draw.get::<ampere>(), 0.0);
        assert_relative_eq!(
            state.battery_remaining_capacity.get::<milliampere_hour>(),
            3999.990722656
        );
        assert_relative_eq!(state.fuel_remaining.get::<liter>(), -1.0 / OUNCES_PER_LITER);
    }

    #[test]
    fn parses_boolean_fields() {
        let state = decode_simulator_state(SIM_STATE_RESPONSE).unwrap();

        assert!(!state.is_locked);
        assert!(!state.has_lost_components);
        assert!(state.an_engine_is_running);
        assert!(!state.is_touching_ground);
        assert!(state.flight_axis_controller_is_active);
    }

    #[test]
    fn parses_status_field() {
        let state = decode_simulator_state(SIM_STATE_RESPONSE).unwrap();
        assert_eq!(state.current_aircraft_status, "CAS-WAITINGTOLAUNCH");
    }
}
