use log::debug;

use crate::BridgeError;
use crate::SimulatorState;
use crate::unit_types::*;

#[cfg(feature = "uom")]
use uom::si::{
    acceleration::meter_per_second_squared, angle::degree, angular_velocity::degree_per_second,
    electric_charge::milliampere_hour, electric_current::ampere, electric_potential::volt,
    length::meter, time::second, velocity::meter_per_second, volume::liter,
};

#[cfg(feature = "uom")]
pub const OUNCES_PER_LITER: f32 = 33.814;

// Converter functions: wrap f32 into appropriate types based on feature flag

#[cfg(feature = "uom")]
fn to_velocity(v: f32) -> Velocity {
    Velocity::new::<meter_per_second>(v)
}
#[cfg(not(feature = "uom"))]
fn to_velocity(v: f32) -> Velocity {
    v
}

#[cfg(feature = "uom")]
fn to_length(v: f32) -> Length {
    Length::new::<meter>(v)
}
#[cfg(not(feature = "uom"))]
fn to_length(v: f32) -> Length {
    v
}

#[cfg(feature = "uom")]
fn to_angular_velocity(v: f32) -> AngularVelocity {
    AngularVelocity::new::<degree_per_second>(v)
}
#[cfg(not(feature = "uom"))]
fn to_angular_velocity(v: f32) -> AngularVelocity {
    v
}

#[cfg(feature = "uom")]
fn to_angle(v: f32) -> Angle {
    Angle::new::<degree>(v)
}
#[cfg(not(feature = "uom"))]
fn to_angle(v: f32) -> Angle {
    v
}

#[cfg(feature = "uom")]
fn to_acceleration(v: f32) -> Acceleration {
    Acceleration::new::<meter_per_second_squared>(v)
}
#[cfg(not(feature = "uom"))]
fn to_acceleration(v: f32) -> Acceleration {
    v
}

#[cfg(feature = "uom")]
fn to_electric_potential(v: f32) -> ElectricPotential {
    ElectricPotential::new::<volt>(v)
}
#[cfg(not(feature = "uom"))]
fn to_electric_potential(v: f32) -> ElectricPotential {
    v
}

#[cfg(feature = "uom")]
fn to_electric_current(v: f32) -> ElectricCurrent {
    ElectricCurrent::new::<ampere>(v)
}
#[cfg(not(feature = "uom"))]
fn to_electric_current(v: f32) -> ElectricCurrent {
    v
}

#[cfg(feature = "uom")]
fn to_electric_charge(v: f32) -> ElectricCharge {
    ElectricCharge::new::<milliampere_hour>(v)
}
#[cfg(not(feature = "uom"))]
fn to_electric_charge(v: f32) -> ElectricCharge {
    v
}

#[cfg(feature = "uom")]
fn to_volume(v: f32) -> Volume {
    Volume::new::<liter>(v)
}

#[cfg(feature = "uom")]
fn to_time(v: f32) -> Time {
    Time::new::<second>(v)
}
#[cfg(not(feature = "uom"))]
fn to_time(v: f32) -> Time {
    v
}

/// Parse string to f32 and convert using provided function
fn parse_with<T, F: Fn(f32) -> T>(name: &str, value: &str, convert: F) -> Result<T, BridgeError> {
    let v: f32 = value.parse().map_err(|e| BridgeError::Parse {
        field: name.to_string(),
        message: format!("{}", e),
    })?;
    Ok(convert(v))
}

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
                open_tag = key.clone();
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
            state.current_physics_time = parse_with(name, value, to_time)?;
        }
        "m-currentPhysicsSpeedMultiplier" => {
            state.current_physics_speed_multiplier = parse_f32(name, value)?;
        }
        "m-airspeed-MPS" => {
            state.airspeed = parse_with(name, value, to_velocity)?;
        }
        "m-altitudeASL-MTR" => {
            state.altitude_asl = parse_with(name, value, to_length)?;
        }
        "m-altitudeAGL-MTR" => {
            state.altitude_agl = parse_with(name, value, to_length)?;
        }
        "m-groundspeed-MPS" => {
            state.groundspeed = parse_with(name, value, to_velocity)?;
        }
        "m-pitchRate-DEGpSEC" => {
            state.pitch_rate = parse_with(name, value, to_angular_velocity)?;
        }
        "m-rollRate-DEGpSEC" => {
            state.roll_rate = parse_with(name, value, to_angular_velocity)?;
        }
        "m-yawRate-DEGpSEC" => {
            state.yaw_rate = parse_with(name, value, to_angular_velocity)?;
        }
        "m-azimuth-DEG" => {
            state.azimuth = parse_with(name, value, to_angle)?;
        }
        "m-inclination-DEG" => {
            state.inclination = parse_with(name, value, to_angle)?;
        }
        "m-roll-DEG" => {
            state.roll = parse_with(name, value, to_angle)?;
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
            state.aircraft_position_x = parse_with(name, value, to_length)?;
        }
        "m-aircraftPositionY-MTR" => {
            state.aircraft_position_y = parse_with(name, value, to_length)?;
        }
        "m-velocityWorldU-MPS" => {
            state.velocity_world_u = parse_with(name, value, to_velocity)?;
        }
        "m-velocityWorldV-MPS" => {
            state.velocity_world_v = parse_with(name, value, to_velocity)?;
        }
        "m-velocityWorldW-MPS" => {
            state.velocity_world_w = parse_with(name, value, to_velocity)?;
        }
        "m-velocityBodyU-MPS" => {
            state.velocity_body_u = parse_with(name, value, to_velocity)?;
        }
        "m-velocityBodyV-MPS" => {
            state.velocity_body_v = parse_with(name, value, to_velocity)?;
        }
        "m-velocityBodyW-MPS" => {
            state.velocity_body_w = parse_with(name, value, to_velocity)?;
        }
        "m-accelerationWorldAX-MPS2" => {
            state.acceleration_world_ax = parse_with(name, value, to_acceleration)?;
        }
        "m-accelerationWorldAY-MPS2" => {
            state.acceleration_world_ay = parse_with(name, value, to_acceleration)?;
        }
        "m-accelerationWorldAZ-MPS2" => {
            state.acceleration_world_az = parse_with(name, value, to_acceleration)?;
        }
        "m-accelerationBodyAX-MPS2" => {
            state.acceleration_body_ax = parse_with(name, value, to_acceleration)?;
        }
        "m-accelerationBodyAY-MPS2" => {
            state.acceleration_body_ay = parse_with(name, value, to_acceleration)?;
        }
        "m-accelerationBodyAZ-MPS2" => {
            state.acceleration_body_az = parse_with(name, value, to_acceleration)?;
        }
        "m-windX-MPS" => {
            state.wind_x = parse_with(name, value, to_velocity)?;
        }
        "m-windY-MPS" => {
            state.wind_y = parse_with(name, value, to_velocity)?;
        }
        "m-windZ-MPS" => {
            state.wind_z = parse_with(name, value, to_velocity)?;
        }
        "m-propRPM" => {
            state.prop_rpm = parse_f32(name, value)?;
        }
        "m-heliMainRotorRPM" => {
            state.heli_main_rotor_rpm = parse_f32(name, value)?;
        }
        "m-batteryVoltage-VOLTS" => {
            state.battery_voltage = parse_with(name, value, to_electric_potential)?;
        }
        "m-batteryCurrentDraw-AMPS" => {
            state.battery_current_draw = parse_with(name, value, to_electric_current)?;
        }
        "m-batteryRemainingCapacity-MAH" => {
            state.battery_remaining_capacity = parse_with(name, value, to_electric_charge)?;
        }
        "m-fuelRemaining-OZ" => {
            state.fuel_remaining = parse_fuel(name, value)?;
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

/// Parse fuel: convert ounces to liters with uom, keep raw value without
#[cfg(feature = "uom")]
fn parse_fuel(name: &str, value: &str) -> Result<Volume, BridgeError> {
    parse_with(name, value, |v| to_volume(v / OUNCES_PER_LITER))
}

/// Parse fuel: keep raw ounces value without uom
#[cfg(not(feature = "uom"))]
fn parse_fuel(name: &str, value: &str) -> Result<Volume, BridgeError> {
    parse_f32(name, value)
}

fn parse_bool(name: &str, value: &str) -> Result<bool, BridgeError> {
    value.parse().map_err(|e| BridgeError::Parse {
        field: name.to_string(),
        message: format!("{}", e),
    })
}

#[cfg(test)]
mod tests;
