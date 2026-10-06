//! Tests for the decoder module.
//!
//! Organized into submodules:
//! - `extract_element`: Tests for XML element extraction
//! - `for_each_leaf`: Tests for the leaf-element scanner
//! - `decode_state_fields`: Tests for full simulator state decoding
//! - `error_handling`: Tests for parse error handling

// Expected values are copied verbatim from the XML fixture.
#![allow(clippy::excessive_precision)]

use approx::assert_relative_eq;

use super::*;

static SIM_STATE_RESPONSE: &str = include_str!("../../testdata/responses/return-data-200.xml");

// ============================================================================
// extract_element Tests
// ============================================================================

mod extract_element {
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
    fn returns_error_for_too_many_channels() {
        let xml = "<item>0.5</item>".repeat(13);
        let result = decode_simulator_state(&xml);

        match result {
            Err(BridgeError::Parse { field, .. }) => assert_eq!(field, "channel[12]"),
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
// for_each_leaf Tests
// ============================================================================

mod for_each_leaf {
    use super::*;

    fn collect(xml: &str) -> Vec<(String, String)> {
        let mut leaves = Vec::new();
        for_each_leaf(xml, |tag, content| {
            leaves.push((tag.to_string(), content.to_string()));
            Ok(())
        })
        .unwrap();
        leaves
    }

    #[test]
    fn yields_leaf_elements_only() {
        let leaves = collect("<?xml version='1.0'?><a><b>1</b><c>two</c></a>");
        assert_eq!(
            leaves,
            vec![("b".into(), "1".into()), ("c".into(), "two".into())]
        );
    }

    #[test]
    fn strips_attributes_from_tag() {
        let leaves = collect(r#"<m-airspeed-MPS xsi:type="xsd:double">1.5</m-airspeed-MPS>"#);
        assert_eq!(leaves, vec![("m-airspeed-MPS".into(), "1.5".into())]);
    }

    #[test]
    fn strips_attributes_after_any_whitespace() {
        for xml in [
            "<a\tx=\"1\">1</a>",
            "<a\nx=\"1\">1</a>",
            "<a\r\n  x=\"1\">1</a>",
        ] {
            assert_eq!(collect(xml), vec![("a".into(), "1".into())], "{xml:?}");
        }
    }

    #[test]
    fn yields_empty_content() {
        assert_eq!(collect("<a></a>"), vec![("a".into(), "".into())]);
    }

    #[test]
    fn skips_empty_element_with_attributes() {
        let leaves = collect(r#"<a xsi:nil="true"></a><b>2</b>"#);
        assert_eq!(leaves, vec![("b".into(), "2".into())]);
    }

    #[test]
    fn returns_empty_for_no_tags() {
        assert!(collect("plain text").is_empty());
    }

    #[test]
    fn propagates_callback_error() {
        let result = for_each_leaf("<a>1</a><b>2</b>", |tag, _| {
            if tag == "b" {
                Err(BridgeError::Parse {
                    field: tag.to_string(),
                    message: "stop".to_string(),
                })
            } else {
                Ok(())
            }
        });
        assert!(matches!(result, Err(BridgeError::Parse { field, .. }) if field == "b"));
    }
}

// ============================================================================
// decode_simulator_state Full Parsing Tests
// ============================================================================

mod decode_state_fields {
    use super::*;

    /// Each numeric tag gets a unique value so swapped mappings are caught.
    #[test]
    fn maps_each_numeric_tag_to_its_field() {
        type Getter = fn(&SimulatorState) -> f32;
        let cases: &[(&str, Getter)] = &[
            ("m-currentPhysicsTime-SEC", |s| s.current_physics_time_s),
            ("m-currentPhysicsSpeedMultiplier", |s| {
                s.current_physics_speed_multiplier
            }),
            ("m-airspeed-MPS", |s| s.airspeed_mps),
            ("m-altitudeASL-MTR", |s| s.altitude_asl_m),
            ("m-altitudeAGL-MTR", |s| s.altitude_agl_m),
            ("m-groundspeed-MPS", |s| s.groundspeed_mps),
            ("m-pitchRate-DEGpSEC", |s| s.pitch_rate_dps),
            ("m-rollRate-DEGpSEC", |s| s.roll_rate_dps),
            ("m-yawRate-DEGpSEC", |s| s.yaw_rate_dps),
            ("m-azimuth-DEG", |s| s.azimuth_deg),
            ("m-inclination-DEG", |s| s.inclination_deg),
            ("m-roll-DEG", |s| s.roll_deg),
            ("m-orientationQuaternion-X", |s| s.orientation.x),
            ("m-orientationQuaternion-Y", |s| s.orientation.y),
            ("m-orientationQuaternion-Z", |s| s.orientation.z),
            ("m-orientationQuaternion-W", |s| s.orientation.w),
            ("m-aircraftPositionX-MTR", |s| s.aircraft_position_x_m),
            ("m-aircraftPositionY-MTR", |s| s.aircraft_position_y_m),
            ("m-velocityWorldU-MPS", |s| s.velocity_world_mps.x),
            ("m-velocityWorldV-MPS", |s| s.velocity_world_mps.y),
            ("m-velocityWorldW-MPS", |s| s.velocity_world_mps.z),
            ("m-velocityBodyU-MPS", |s| s.velocity_body_mps.x),
            ("m-velocityBodyV-MPS", |s| s.velocity_body_mps.y),
            ("m-velocityBodyW-MPS", |s| s.velocity_body_mps.z),
            ("m-accelerationWorldAX-MPS2", |s| {
                s.acceleration_world_mps2.x
            }),
            ("m-accelerationWorldAY-MPS2", |s| {
                s.acceleration_world_mps2.y
            }),
            ("m-accelerationWorldAZ-MPS2", |s| {
                s.acceleration_world_mps2.z
            }),
            ("m-accelerationBodyAX-MPS2", |s| s.acceleration_body_mps2.x),
            ("m-accelerationBodyAY-MPS2", |s| s.acceleration_body_mps2.y),
            ("m-accelerationBodyAZ-MPS2", |s| s.acceleration_body_mps2.z),
            ("m-windX-MPS", |s| s.wind_mps.x),
            ("m-windY-MPS", |s| s.wind_mps.y),
            ("m-windZ-MPS", |s| s.wind_mps.z),
            ("m-propRPM", |s| s.prop_rpm),
            ("m-heliMainRotorRPM", |s| s.heli_main_rotor_rpm),
            ("m-batteryVoltage-VOLTS", |s| s.battery_voltage_v),
            ("m-batteryCurrentDraw-AMPS", |s| s.battery_current_draw_a),
            ("m-batteryRemainingCapacity-MAH", |s| {
                s.battery_remaining_capacity_mah
            }),
            ("m-fuelRemaining-OZ", |s| s.fuel_remaining_oz),
        ];

        let xml: String = cases
            .iter()
            .enumerate()
            .map(|(i, (tag, _))| format!("<{tag}>{}</{tag}>", i + 1))
            .collect();
        let state = decode_simulator_state(&xml).unwrap();

        for (i, (tag, get)) in cases.iter().enumerate() {
            assert_eq!(get(&state), (i + 1) as f32, "{tag}");
        }
    }

    #[test]
    fn maps_each_boolean_tag_to_its_field() {
        type Getter = fn(&SimulatorState) -> bool;
        let cases: &[(&str, Getter)] = &[
            ("m-isLocked", |s| s.is_locked),
            ("m-hasLostComponents", |s| s.has_lost_components),
            ("m-anEngineIsRunning", |s| s.an_engine_is_running),
            ("m-isTouchingGround", |s| s.is_touching_ground),
            ("m-flightAxisControllerIsActive", |s| {
                s.flight_axis_controller_is_active
            }),
            ("m-resetButtonHasBeenPressed", |s| {
                s.reset_button_has_been_pressed
            }),
        ];

        // Set one flag at a time so a swapped mapping shows up as a wrong field
        for (tag, get) in cases {
            let state = decode_simulator_state(&format!("<{tag}>true</{tag}>")).unwrap();
            assert!(get(&state), "{tag}");
            let set = cases.iter().filter(|(_, g)| g(&state)).count();
            assert_eq!(set, 1, "{tag} set other flags");
        }
    }

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

        assert_relative_eq!(state.current_physics_time_s, 72263.411813672);
        assert_relative_eq!(state.current_physics_speed_multiplier, 1.0);
    }

    #[test]
    fn parses_velocity_fields() {
        let state = decode_simulator_state(SIM_STATE_RESPONSE).unwrap();

        assert_relative_eq!(state.airspeed_mps, 0.040872246);
        assert_relative_eq!(state.groundspeed_mps, 4.6434447540377732E-06);
        assert_relative_eq!(state.velocity_world_mps.x, -2.005582700E-06);
        assert_relative_eq!(state.velocity_world_mps.y, 4.187984814E-06);
        assert_relative_eq!(state.velocity_world_mps.z, 0.040872246);
        assert_relative_eq!(state.velocity_body_mps.x, -0.001089469);
        assert_relative_eq!(state.velocity_body_mps.y, -0.000530726);
        assert_relative_eq!(state.velocity_body_mps.z, 0.040854275);
    }

    #[test]
    fn parses_position_fields() {
        let state = decode_simulator_state(SIM_STATE_RESPONSE).unwrap();

        assert_relative_eq!(state.altitude_asl_m, 1127.370971679);
        assert_relative_eq!(state.altitude_agl_m, 0.266309916);
        assert_relative_eq!(state.aircraft_position_x_m, 5575.680664062);
        assert_relative_eq!(state.aircraft_position_y_m, 1715.962158203);
    }

    #[test]
    fn parses_angular_rate_fields() {
        let state = decode_simulator_state(SIM_STATE_RESPONSE).unwrap();

        assert_relative_eq!(state.pitch_rate_dps, 0.001380353);
        assert_relative_eq!(state.roll_rate_dps, -0.000032227);
        assert_relative_eq!(state.yaw_rate_dps, 0.001473751);
    }

    #[test]
    fn parses_orientation_fields() {
        let state = decode_simulator_state(SIM_STATE_RESPONSE).unwrap();

        assert_relative_eq!(state.azimuth_deg, -89.607055664);
        assert_relative_eq!(state.inclination_deg, 1.533278226);
        assert_relative_eq!(state.roll_deg, -0.747124254);
    }

    #[test]
    fn parses_quaternion_fields() {
        let state = decode_simulator_state(SIM_STATE_RESPONSE).unwrap();

        assert_relative_eq!(state.orientation.x, 0.004899279);
        assert_relative_eq!(state.orientation.y, -0.014053969);
        assert_relative_eq!(state.orientation.z, -0.704661786);
        assert_relative_eq!(state.orientation.w, 0.709387302);
    }

    #[test]
    fn parses_acceleration_fields() {
        let state = decode_simulator_state(SIM_STATE_RESPONSE).unwrap();

        assert_relative_eq!(state.acceleration_world_mps2.x, -0.000483050);
        assert_relative_eq!(state.acceleration_world_mps2.y, 0.001008689);
        assert_relative_eq!(state.acceleration_world_mps2.z, 9.844209671);
        assert_relative_eq!(state.acceleration_body_mps2.x, -0.000176936);
        assert_relative_eq!(state.acceleration_body_mps2.y, -0.000086620);
        assert_relative_eq!(state.acceleration_body_mps2.z, 0.044223785);
    }

    #[test]
    fn parses_wind_fields() {
        let state = decode_simulator_state(SIM_STATE_RESPONSE).unwrap();

        assert_relative_eq!(state.wind_mps.x, 0.0);
        assert_relative_eq!(state.wind_mps.y, 0.0);
        assert_relative_eq!(state.wind_mps.z, 0.0);
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

        assert_relative_eq!(state.battery_voltage_v, 12.599982261);
        assert_relative_eq!(state.battery_current_draw_a, 0.0);
        assert_relative_eq!(state.battery_remaining_capacity_mah, 3999.990722656);
        assert_relative_eq!(state.fuel_remaining_oz, -1.0);
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
