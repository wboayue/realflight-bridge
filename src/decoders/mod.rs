use log::debug;

use crate::BridgeError;
use crate::SimulatorState;

pub fn extract_element(name: &str, xml: &str) -> Option<String> {
    let start_tag = &format!("<{}>", name);
    let end_tag = &format!("</{}>", name);

    let start_pos = xml.find(start_tag)?;
    let end_pos = xml.find(end_tag)?;

    let detail_start = start_pos + start_tag.len();
    if detail_start >= end_pos {
        return None;
    }

    Some(xml[detail_start..end_pos].to_string())
}

enum ParseState {
    FindTag,
    MaybeTag,
    Content,
    OpenTag,
    CloseTag,
}

pub fn decode_simulator_state(xml: &str) -> Result<SimulatorState, BridgeError> {
    let mut state = ParseState::FindTag;
    let mut key = String::new();
    let mut open_tag = String::new();
    let mut content = String::new();

    let mut channel_ndx: usize = 0;
    let mut result = SimulatorState::default();

    for ch in xml.chars() {
        match state {
            ParseState::FindTag if ch == '<' => {
                key.clear();
                state = ParseState::MaybeTag;
            }
            ParseState::FindTag => continue,
            ParseState::MaybeTag if ch == '?' => {
                state = ParseState::FindTag;
            }
            ParseState::MaybeTag if ch == '/' => {
                key.clear();
                state = ParseState::CloseTag;
            }
            ParseState::MaybeTag => {
                key.clear();
                key.push(ch);
                state = ParseState::OpenTag;
            }
            ParseState::OpenTag if ch == '>' => {
                open_tag = std::mem::take(&mut key);
                content.clear();
                state = ParseState::Content;
            }
            ParseState::OpenTag => {
                key.push(ch);
            }
            ParseState::Content if ch == '<' => {
                state = ParseState::MaybeTag;
            }
            ParseState::Content => {
                content.push(ch);
            }
            ParseState::CloseTag if ch == '>' => {
                if open_tag == key {
                    if open_tag == "item" {
                        let value = content.parse::<f32>().map_err(|e| BridgeError::Parse {
                            field: format!("channel[{}]", channel_ndx),
                            message: format!("{}", e),
                        })?;
                        result.previous_inputs.channels[channel_ndx] = value;
                        channel_ndx += 1;
                    } else {
                        decode_state_field(&mut result, &open_tag, &content)?;
                    }
                }
                state = ParseState::FindTag;
                key.clear();
                content.clear();
            }
            ParseState::CloseTag => {
                key.push(ch);
            }
        }
    }

    Ok(result)
}

fn decode_state_field(
    state: &mut SimulatorState,
    name: &str,
    value: &str,
) -> Result<(), BridgeError> {
    match name {
        "m-currentPhysicsTime-SEC" => {
            state.current_physics_time_s = parse_f32(name, value)?;
        }
        "m-currentPhysicsSpeedMultiplier" => {
            state.current_physics_speed_multiplier = parse_f32(name, value)?;
        }
        "m-airspeed-MPS" => {
            state.airspeed_mps = parse_f32(name, value)?;
        }
        "m-altitudeASL-MTR" => {
            state.altitude_asl_m = parse_f32(name, value)?;
        }
        "m-altitudeAGL-MTR" => {
            state.altitude_agl_m = parse_f32(name, value)?;
        }
        "m-groundspeed-MPS" => {
            state.groundspeed_mps = parse_f32(name, value)?;
        }
        "m-pitchRate-DEGpSEC" => {
            state.pitch_rate_dps = parse_f32(name, value)?;
        }
        "m-rollRate-DEGpSEC" => {
            state.roll_rate_dps = parse_f32(name, value)?;
        }
        "m-yawRate-DEGpSEC" => {
            state.yaw_rate_dps = parse_f32(name, value)?;
        }
        "m-azimuth-DEG" => {
            state.azimuth_deg = parse_f32(name, value)?;
        }
        "m-inclination-DEG" => {
            state.inclination_deg = parse_f32(name, value)?;
        }
        "m-roll-DEG" => {
            state.roll_deg = parse_f32(name, value)?;
        }
        "m-orientationQuaternion-X" => {
            state.orientation_quaternion_x = parse_f32(name, value)?;
        }
        "m-orientationQuaternion-Y" => {
            state.orientation_quaternion_y = parse_f32(name, value)?;
        }
        "m-orientationQuaternion-Z" => {
            state.orientation_quaternion_z = parse_f32(name, value)?;
        }
        "m-orientationQuaternion-W" => {
            state.orientation_quaternion_w = parse_f32(name, value)?;
        }
        "m-aircraftPositionX-MTR" => {
            state.aircraft_position_x_m = parse_f32(name, value)?;
        }
        "m-aircraftPositionY-MTR" => {
            state.aircraft_position_y_m = parse_f32(name, value)?;
        }
        "m-velocityWorldU-MPS" => {
            state.velocity_world_u_mps = parse_f32(name, value)?;
        }
        "m-velocityWorldV-MPS" => {
            state.velocity_world_v_mps = parse_f32(name, value)?;
        }
        "m-velocityWorldW-MPS" => {
            state.velocity_world_w_mps = parse_f32(name, value)?;
        }
        "m-velocityBodyU-MPS" => {
            state.velocity_body_u_mps = parse_f32(name, value)?;
        }
        "m-velocityBodyV-MPS" => {
            state.velocity_body_v_mps = parse_f32(name, value)?;
        }
        "m-velocityBodyW-MPS" => {
            state.velocity_body_w_mps = parse_f32(name, value)?;
        }
        "m-accelerationWorldAX-MPS2" => {
            state.acceleration_world_ax_mps2 = parse_f32(name, value)?;
        }
        "m-accelerationWorldAY-MPS2" => {
            state.acceleration_world_ay_mps2 = parse_f32(name, value)?;
        }
        "m-accelerationWorldAZ-MPS2" => {
            state.acceleration_world_az_mps2 = parse_f32(name, value)?;
        }
        "m-accelerationBodyAX-MPS2" => {
            state.acceleration_body_ax_mps2 = parse_f32(name, value)?;
        }
        "m-accelerationBodyAY-MPS2" => {
            state.acceleration_body_ay_mps2 = parse_f32(name, value)?;
        }
        "m-accelerationBodyAZ-MPS2" => {
            state.acceleration_body_az_mps2 = parse_f32(name, value)?;
        }
        "m-windX-MPS" => {
            state.wind_x_mps = parse_f32(name, value)?;
        }
        "m-windY-MPS" => {
            state.wind_y_mps = parse_f32(name, value)?;
        }
        "m-windZ-MPS" => {
            state.wind_z_mps = parse_f32(name, value)?;
        }
        "m-propRPM" => {
            state.prop_rpm = parse_f32(name, value)?;
        }
        "m-heliMainRotorRPM" => {
            state.heli_main_rotor_rpm = parse_f32(name, value)?;
        }
        "m-batteryVoltage-VOLTS" => {
            state.battery_voltage_v = parse_f32(name, value)?;
        }
        "m-batteryCurrentDraw-AMPS" => {
            state.battery_current_draw_a = parse_f32(name, value)?;
        }
        "m-batteryRemainingCapacity-MAH" => {
            state.battery_remaining_capacity_mah = parse_f32(name, value)?;
        }
        "m-fuelRemaining-OZ" => {
            state.fuel_remaining_oz = parse_f32(name, value)?;
        }
        "m-isLocked" => {
            state.is_locked = parse_bool(name, value)?;
        }
        "m-hasLostComponents" => {
            state.has_lost_components = parse_bool(name, value)?;
        }
        "m-anEngineIsRunning" => {
            state.an_engine_is_running = parse_bool(name, value)?;
        }
        "m-isTouchingGround" => {
            state.is_touching_ground = parse_bool(name, value)?;
        }
        "m-flightAxisControllerIsActive" => {
            state.flight_axis_controller_is_active = parse_bool(name, value)?;
        }
        "m-currentAircraftStatus" => {
            state.current_aircraft_status = value.to_string();
        }
        "m-resetButtonHasBeenPressed" => {
            state.reset_button_has_been_pressed = parse_bool(name, value)?;
        }
        _ => {
            debug!("Unexpected attribute {}: {}", name, value);
        }
    }
    Ok(())
}

fn parse_f32(name: &str, value: &str) -> Result<f32, BridgeError> {
    value.parse().map_err(|e| BridgeError::Parse {
        field: name.to_string(),
        message: format!("{}", e),
    })
}

fn parse_bool(name: &str, value: &str) -> Result<bool, BridgeError> {
    value.parse().map_err(|e| BridgeError::Parse {
        field: name.to_string(),
        message: format!("{}", e),
    })
}

#[cfg(test)]
mod tests;
