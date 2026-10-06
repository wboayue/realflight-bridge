use std::fmt::Display;
use std::str::FromStr;

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

/// Walks `xml` and calls `on_leaf(tag, content)` for every leaf element
/// (an element whose close tag matches the most recent open tag).
///
/// Lightweight, allocation-reusing scanner tailored to RealFlight responses.
/// Attributes (anything after the first ASCII whitespace in a tag) are dropped, so
/// `tag` is the bare element name. Empty elements that carry attributes (e.g.
/// `xsi:nil="true"`) are skipped rather than reported with empty content.
fn for_each_leaf<F>(xml: &str, mut on_leaf: F) -> Result<(), BridgeError>
where
    F: FnMut(&str, &str) -> Result<(), BridgeError>,
{
    let mut state = ParseState::FindTag;
    let mut key = String::new();
    let mut open_tag = String::new();
    let mut open_tag_has_attrs = false;
    let mut content = String::new();

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
                // XML only allows ASCII whitespace before attributes
                open_tag_has_attrs = match key.bytes().position(|b| b.is_ascii_whitespace()) {
                    Some(end) => {
                        key.truncate(end);
                        true
                    }
                    None => false,
                };
                // Swap rather than take so both buffers keep their capacity
                std::mem::swap(&mut open_tag, &mut key);
                key.clear();
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
                if open_tag == key && !(open_tag_has_attrs && content.is_empty()) {
                    on_leaf(&open_tag, &content)?;
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

    Ok(())
}

pub fn decode_simulator_state(xml: &str) -> Result<SimulatorState, BridgeError> {
    let mut result = SimulatorState::default();
    let mut channel_ndx: usize = 0;

    for_each_leaf(xml, |tag, content| {
        if tag == "item" {
            decode_channel(&mut result, channel_ndx, content)?;
            channel_ndx += 1;
            Ok(())
        } else {
            decode_state_field(&mut result, tag, content)
        }
    })?;

    Ok(result)
}

fn decode_channel(state: &mut SimulatorState, ndx: usize, value: &str) -> Result<(), BridgeError> {
    let channel =
        state
            .previous_inputs
            .channels
            .get_mut(ndx)
            .ok_or_else(|| BridgeError::Parse {
                field: format!("channel[{}]", ndx),
                message: "too many channel values".to_string(),
            })?;
    *channel = parse(format_args!("channel[{}]", ndx), value)?;
    Ok(())
}

fn decode_state_field(
    state: &mut SimulatorState,
    name: &str,
    value: &str,
) -> Result<(), BridgeError> {
    match name {
        "m-currentPhysicsTime-SEC" => state.current_physics_time_s = parse(name, value)?,
        "m-currentPhysicsSpeedMultiplier" => {
            state.current_physics_speed_multiplier = parse(name, value)?
        }
        "m-airspeed-MPS" => state.airspeed_mps = parse(name, value)?,
        "m-altitudeASL-MTR" => state.altitude_asl_m = parse(name, value)?,
        "m-altitudeAGL-MTR" => state.altitude_agl_m = parse(name, value)?,
        "m-groundspeed-MPS" => state.groundspeed_mps = parse(name, value)?,
        "m-pitchRate-DEGpSEC" => state.pitch_rate_dps = parse(name, value)?,
        "m-rollRate-DEGpSEC" => state.roll_rate_dps = parse(name, value)?,
        "m-yawRate-DEGpSEC" => state.yaw_rate_dps = parse(name, value)?,
        "m-azimuth-DEG" => state.azimuth_deg = parse(name, value)?,
        "m-inclination-DEG" => state.inclination_deg = parse(name, value)?,
        "m-roll-DEG" => state.roll_deg = parse(name, value)?,
        "m-orientationQuaternion-X" => state.orientation.x = parse(name, value)?,
        "m-orientationQuaternion-Y" => state.orientation.y = parse(name, value)?,
        "m-orientationQuaternion-Z" => state.orientation.z = parse(name, value)?,
        "m-orientationQuaternion-W" => state.orientation.w = parse(name, value)?,
        "m-aircraftPositionX-MTR" => state.aircraft_position_x_m = parse(name, value)?,
        "m-aircraftPositionY-MTR" => state.aircraft_position_y_m = parse(name, value)?,
        "m-velocityWorldU-MPS" => state.velocity_world_mps.x = parse(name, value)?,
        "m-velocityWorldV-MPS" => state.velocity_world_mps.y = parse(name, value)?,
        "m-velocityWorldW-MPS" => state.velocity_world_mps.z = parse(name, value)?,
        "m-velocityBodyU-MPS" => state.velocity_body_mps.x = parse(name, value)?,
        "m-velocityBodyV-MPS" => state.velocity_body_mps.y = parse(name, value)?,
        "m-velocityBodyW-MPS" => state.velocity_body_mps.z = parse(name, value)?,
        "m-accelerationWorldAX-MPS2" => state.acceleration_world_mps2.x = parse(name, value)?,
        "m-accelerationWorldAY-MPS2" => state.acceleration_world_mps2.y = parse(name, value)?,
        "m-accelerationWorldAZ-MPS2" => state.acceleration_world_mps2.z = parse(name, value)?,
        "m-accelerationBodyAX-MPS2" => state.acceleration_body_mps2.x = parse(name, value)?,
        "m-accelerationBodyAY-MPS2" => state.acceleration_body_mps2.y = parse(name, value)?,
        "m-accelerationBodyAZ-MPS2" => state.acceleration_body_mps2.z = parse(name, value)?,
        "m-windX-MPS" => state.wind_mps.x = parse(name, value)?,
        "m-windY-MPS" => state.wind_mps.y = parse(name, value)?,
        "m-windZ-MPS" => state.wind_mps.z = parse(name, value)?,
        "m-propRPM" => state.prop_rpm = parse(name, value)?,
        "m-heliMainRotorRPM" => state.heli_main_rotor_rpm = parse(name, value)?,
        "m-batteryVoltage-VOLTS" => state.battery_voltage_v = parse(name, value)?,
        "m-batteryCurrentDraw-AMPS" => state.battery_current_draw_a = parse(name, value)?,
        "m-batteryRemainingCapacity-MAH" => {
            state.battery_remaining_capacity_mah = parse(name, value)?
        }
        "m-fuelRemaining-OZ" => state.fuel_remaining_oz = parse(name, value)?,
        "m-isLocked" => state.is_locked = parse(name, value)?,
        "m-hasLostComponents" => state.has_lost_components = parse(name, value)?,
        "m-anEngineIsRunning" => state.an_engine_is_running = parse(name, value)?,
        "m-isTouchingGround" => state.is_touching_ground = parse(name, value)?,
        "m-flightAxisControllerIsActive" => {
            state.flight_axis_controller_is_active = parse(name, value)?
        }
        "m-currentAircraftStatus" => state.current_aircraft_status = value.to_string(),
        "m-resetButtonHasBeenPressed" => state.reset_button_has_been_pressed = parse(name, value)?,
        _ => debug!("Unexpected attribute {}: {}", name, value),
    }
    Ok(())
}

/// Parse `value` into `T`, reporting `field` on failure. `field` is only
/// formatted on error, so `format_args!` can be passed without allocating.
fn parse<T>(field: impl Display, value: &str) -> Result<T, BridgeError>
where
    T: FromStr,
    T::Err: Display,
{
    value.parse().map_err(|e: T::Err| BridgeError::Parse {
        field: field.to_string(),
        message: e.to_string(),
    })
}

#[cfg(test)]
mod tests;
