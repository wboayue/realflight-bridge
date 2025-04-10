use std::error::Error;

use log::debug;
#[cfg(feature = "uom")]
use uom::si::acceleration::meter_per_second_squared;
#[cfg(feature = "uom")]
use uom::si::angle::degree;
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
#[cfg(feature = "uom")]
use uom::si::{angular_velocity::degree_per_second, f32::*};

use super::SimulatorState;

#[cfg(feature = "uom")]
pub const OUNCES_PER_LITER: f32 = 33.814;

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

pub fn decode_simulator_state(xml: &str) -> Result<SimulatorState, Box<dyn Error>> {
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
                        let value = content.parse::<f32>().map_err(|e| {
                            format!(
                                "Failed to parse channel {}: {}. {}",
                                channel_ndx, content, e
                            )
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

#[cfg(feature = "uom")]
fn decode_state_field(
    state: &mut SimulatorState,
    name: &str,
    value: &str,
) -> Result<(), Box<dyn Error>> {
    match name {
        "m-currentPhysicsTime-SEC" => {
            state.current_physics_time = as_time(name, value)?;
        }
        "m-currentPhysicsSpeedMultiplier" => {
            state.current_physics_speed_multiplier = as_f32(name, value)?;
        }
        "m-airspeed-MPS" => {
            state.airspeed = as_velocity(name, value)?;
        }
        "m-altitudeASL-MTR" => {
            state.altitude_asl = as_length(name, value)?;
        }
        "m-altitudeAGL-MTR" => {
            state.altitude_agl = as_length(name, value)?;
        }
        "m-groundspeed-MPS" => {
            state.groundspeed = as_velocity(name, value)?;
        }
        "m-pitchRate-DEGpSEC" => {
            state.pitch_rate = as_angular_velocity(name, value)?;
        }
        "m-rollRate-DEGpSEC" => {
            state.roll_rate = as_angular_velocity(name, value)?;
        }
        "m-yawRate-DEGpSEC" => {
            state.yaw_rate = as_angular_velocity(name, value)?;
        }
        "m-azimuth-DEG" => {
            state.azimuth = as_angle(name, value)?;
        }
        "m-inclination-DEG" => {
            state.inclination = as_angle(name, value)?;
        }
        "m-roll-DEG" => {
            state.roll = as_angle(name, value)?;
        }
        "m-orientationQuaternion-X" => {
            state.orientation_quaternion_x = as_f32(name, value)?;
        }
        "m-orientationQuaternion-Y" => {
            state.orientation_quaternion_y = as_f32(name, value)?;
        }
        "m-orientationQuaternion-Z" => {
            state.orientation_quaternion_z = as_f32(name, value)?;
        }
        "m-orientationQuaternion-W" => {
            state.orientation_quaternion_w = as_f32(name, value)?;
        }
        "m-aircraftPositionX-MTR" => {
            state.aircraft_position_x = as_length(name, value)?;
        }
        "m-aircraftPositionY-MTR" => {
            state.aircraft_position_y = as_length(name, value)?;
        }
        "m-velocityWorldU-MPS" => {
            state.velocity_world_u = as_velocity(name, value)?;
        }
        "m-velocityWorldV-MPS" => {
            state.velocity_world_v = as_velocity(name, value)?;
        }
        "m-velocityWorldW-MPS" => {
            state.velocity_world_w = as_velocity(name, value)?;
        }
        "m-velocityBodyU-MPS" => {
            state.velocity_body_u = as_velocity(name, value)?;
        }
        "m-velocityBodyV-MPS" => {
            state.velocity_body_v = as_velocity(name, value)?;
        }
        "m-velocityBodyW-MPS" => {
            state.velocity_body_w = as_velocity(name, value)?;
        }
        "m-accelerationWorldAX-MPS2" => {
            state.acceleration_world_ax = as_acceleration(name, value)?;
        }
        "m-accelerationWorldAY-MPS2" => {
            state.acceleration_world_ay = as_acceleration(name, value)?;
        }
        "m-accelerationWorldAZ-MPS2" => {
            state.acceleration_world_az = as_acceleration(name, value)?;
        }
        "m-accelerationBodyAX-MPS2" => {
            state.acceleration_body_ax = as_acceleration(name, value)?;
        }
        "m-accelerationBodyAY-MPS2" => {
            state.acceleration_body_ay = as_acceleration(name, value)?;
        }
        "m-accelerationBodyAZ-MPS2" => {
            state.acceleration_body_az = as_acceleration(name, value)?;
        }
        "m-windX-MPS" => {
            state.wind_x = as_velocity(name, value)?;
        }
        "m-windY-MPS" => {
            state.wind_y = as_velocity(name, value)?;
        }
        "m-windZ-MPS" => {
            state.wind_z = as_velocity(name, value)?;
        }
        "m-propRPM" => {
            state.prop_rpm = as_f32(name, value)?;
        }
        "m-heliMainRotorRPM" => {
            state.heli_main_rotor_rpm = as_f32(name, value)?;
        }
        "m-batteryVoltage-VOLTS" => {
            state.battery_voltage = as_electrical_potential(name, value)?;
        }
        "m-batteryCurrentDraw-AMPS" => {
            state.battery_current_draw = as_electrical_current(name, value)?;
        }
        "m-batteryRemainingCapacity-MAH" => {
            state.battery_remaining_capacity = as_electrical_charge(name, value)?;
        }
        "m-fuelRemaining-OZ" => {
            state.fuel_remaining = as_volume(name, value, Some(1.0 / OUNCES_PER_LITER))?;
        }
        "m-isLocked" => {
            state.is_locked = as_bool(name, value)?;
        }
        "m-hasLostComponents" => {
            state.has_lost_components = as_bool(name, value)?;
        }
        "m-anEngineIsRunning" => {
            state.an_engine_is_running = as_bool(name, value)?;
        }
        "m-isTouchingGround" => {
            state.is_touching_ground = as_bool(name, value)?;
        }
        "m-flightAxisControllerIsActive" => {
            state.flight_axis_controller_is_active = as_bool(name, value)?;
        }
        "m-currentAircraftStatus" => {
            state.current_aircraft_status = value.to_string();
        }
        "m-resetButtonHasBeenPressed" => {
            state.reset_button_has_been_pressed = as_bool(name, value)?;
        }
        _ => {
            debug!("Unexpected attribute {}: {}", name, value);
        }
    }
    Ok(())
}

#[cfg(not(feature = "uom"))]
fn decode_state_field(
    state: &mut SimulatorState,
    name: &str,
    value: &str,
) -> Result<(), Box<dyn Error>> {
    match name {
        "m-currentPhysicsTime-SEC" => {
            state.current_physics_time = as_f32(name, value)?;
        }
        "m-currentPhysicsSpeedMultiplier" => {
            state.current_physics_speed_multiplier = as_f32(name, value)?;
        }
        "m-airspeed-MPS" => {
            state.airspeed = as_f32(name, value)?;
        }
        "m-altitudeASL-MTR" => {
            state.altitude_asl = as_f32(name, value)?;
        }
        "m-altitudeAGL-MTR" => {
            state.altitude_agl = as_f32(name, value)?;
        }
        "m-groundspeed-MPS" => {
            state.groundspeed = as_f32(name, value)?;
        }
        "m-pitchRate-DEGpSEC" => {
            state.pitch_rate = as_f32(name, value)?;
        }
        "m-rollRate-DEGpSEC" => {
            state.roll_rate = as_f32(name, value)?;
        }
        "m-yawRate-DEGpSEC" => {
            state.yaw_rate = as_f32(name, value)?;
        }
        "m-azimuth-DEG" => {
            state.azimuth = as_f32(name, value)?;
        }
        "m-inclination-DEG" => {
            state.inclination = as_f32(name, value)?;
        }
        "m-roll-DEG" => {
            state.roll = as_f32(name, value)?;
        }
        "m-orientationQuaternion-X" => {
            state.orientation_quaternion_x = as_f32(name, value)?;
        }
        "m-orientationQuaternion-Y" => {
            state.orientation_quaternion_y = as_f32(name, value)?;
        }
        "m-orientationQuaternion-Z" => {
            state.orientation_quaternion_z = as_f32(name, value)?;
        }
        "m-orientationQuaternion-W" => {
            state.orientation_quaternion_w = as_f32(name, value)?;
        }
        "m-aircraftPositionX-MTR" => {
            state.aircraft_position_x = as_f32(name, value)?;
        }
        "m-aircraftPositionY-MTR" => {
            state.aircraft_position_y = as_f32(name, value)?;
        }
        "m-velocityWorldU-MPS" => {
            state.velocity_world_u = as_f32(name, value)?;
        }
        "m-velocityWorldV-MPS" => {
            state.velocity_world_v = as_f32(name, value)?;
        }
        "m-velocityWorldW-MPS" => {
            state.velocity_world_w = as_f32(name, value)?;
        }
        "m-velocityBodyU-MPS" => {
            state.velocity_body_u = as_f32(name, value)?;
        }
        "m-velocityBodyV-MPS" => {
            state.velocity_body_v = as_f32(name, value)?;
        }
        "m-velocityBodyW-MPS" => {
            state.velocity_body_w = as_f32(name, value)?;
        }
        "m-accelerationWorldAX-MPS2" => {
            state.acceleration_world_ax = as_f32(name, value)?;
        }
        "m-accelerationWorldAY-MPS2" => {
            state.acceleration_world_ay = as_f32(name, value)?;
        }
        "m-accelerationWorldAZ-MPS2" => {
            state.acceleration_world_az = as_f32(name, value)?;
        }
        "m-accelerationBodyAX-MPS2" => {
            state.acceleration_body_ax = as_f32(name, value)?;
        }
        "m-accelerationBodyAY-MPS2" => {
            state.acceleration_body_ay = as_f32(name, value)?;
        }
        "m-accelerationBodyAZ-MPS2" => {
            state.acceleration_body_az = as_f32(name, value)?;
        }
        "m-windX-MPS" => {
            state.wind_x = as_f32(name, value)?;
        }
        "m-windY-MPS" => {
            state.wind_y = as_f32(name, value)?;
        }
        "m-windZ-MPS" => {
            state.wind_z = as_f32(name, value)?;
        }
        "m-propRPM" => {
            state.prop_rpm = as_f32(name, value)?;
        }
        "m-heliMainRotorRPM" => {
            state.heli_main_rotor_rpm = as_f32(name, value)?;
        }
        "m-batteryVoltage-VOLTS" => {
            state.battery_voltage = as_f32(name, value)?;
        }
        "m-batteryCurrentDraw-AMPS" => {
            state.battery_current_draw = as_f32(name, value)?;
        }
        "m-batteryRemainingCapacity-MAH" => {
            state.battery_remaining_capacity = as_f32(name, value)?;
        }
        "m-fuelRemaining-OZ" => {
            state.fuel_remaining = as_f32(name, value)?;
        }
        "m-isLocked" => {
            state.is_locked = as_bool(name, value)?;
        }
        "m-hasLostComponents" => {
            state.has_lost_components = as_bool(name, value)?;
        }
        "m-anEngineIsRunning" => {
            state.an_engine_is_running = as_bool(name, value)?;
        }
        "m-isTouchingGround" => {
            state.is_touching_ground = as_bool(name, value)?;
        }
        "m-flightAxisControllerIsActive" => {
            state.flight_axis_controller_is_active = as_bool(name, value)?;
        }
        "m-currentAircraftStatus" => {
            state.current_aircraft_status = value.to_string();
        }
        "m-resetButtonHasBeenPressed" => {
            state.reset_button_has_been_pressed = as_bool(name, value)?;
        }
        _ => {
            debug!("Unexpected attribute {}: {}", name, value);
        }
    }
    Ok(())
}

#[cfg(feature = "uom")]
fn as_time(name: &str, value: &str) -> Result<Time, Box<dyn Error>> {
    let result = value.parse();
    match result {
        Ok(value) => Ok(Time::new::<second>(value)),
        Err(e) => Err(format!("Failed to parse time {}: {}. {}", name, value, e).into()),
    }
}

fn as_f32(name: &str, value: &str) -> Result<f32, Box<dyn Error>> {
    let result = value.parse();
    match result {
        Ok(value) => Ok(value),
        Err(e) => Err(format!("Failed to parse f32 {}: {}. {}", name, value, e).into()),
    }
}

#[cfg(feature = "uom")]
fn as_velocity(name: &str, value: &str) -> Result<Velocity, Box<dyn Error>> {
    let result = value.parse();
    match result {
        Ok(value) => Ok(Velocity::new::<meter_per_second>(value)),
        Err(e) => Err(format!("Failed to parse Velocity {}: {}. {}", name, value, e).into()),
    }
}

#[cfg(feature = "uom")]
fn as_angular_velocity(name: &str, value: &str) -> Result<AngularVelocity, Box<dyn Error>> {
    let result = value.parse();
    match result {
        Ok(value) => Ok(AngularVelocity::new::<degree_per_second>(value)),
        Err(e) => Err(format!("Failed to parse AngularVelocity {}: {}. {}", name, value, e).into()),
    }
}

#[cfg(feature = "uom")]
fn as_length(name: &str, value: &str) -> Result<Length, Box<dyn Error>> {
    let result = value.parse();
    match result {
        Ok(value) => Ok(Length::new::<meter>(value)),
        Err(e) => Err(format!("Failed to parse Length {}: {}. {}", name, value, e).into()),
    }
}

#[cfg(feature = "uom")]
fn as_angle(name: &str, value: &str) -> Result<Angle, Box<dyn Error>> {
    let result = value.parse();
    match result {
        Ok(value) => Ok(Angle::new::<degree>(value)),
        Err(e) => Err(format!("Failed to parse Angle {}: {}. {}", name, value, e).into()),
    }
}

#[cfg(feature = "uom")]
fn as_acceleration(name: &str, value: &str) -> Result<Acceleration, Box<dyn Error>> {
    let result = value.parse();
    match result {
        Ok(value) => Ok(Acceleration::new::<meter_per_second_squared>(value)),
        Err(e) => Err(format!("Failed to parse Acceleration {}: {}. {}", name, value, e).into()),
    }
}

#[cfg(feature = "uom")]
fn as_electrical_potential(name: &str, value: &str) -> Result<ElectricPotential, Box<dyn Error>> {
    let result = value.parse();
    match result {
        Ok(value) => Ok(ElectricPotential::new::<volt>(value)),
        Err(e) => Err(format!(
            "Failed to parse ElectricPotential {}: {}. {}",
            name, value, e
        )
        .into()),
    }
}

#[cfg(feature = "uom")]
fn as_electrical_current(name: &str, value: &str) -> Result<ElectricCurrent, Box<dyn Error>> {
    let result = value.parse();
    match result {
        Ok(value) => Ok(ElectricCurrent::new::<ampere>(value)),
        Err(e) => Err(format!("Failed to parse ElectricCurrent {}: {}. {}", name, value, e).into()),
    }
}

#[cfg(feature = "uom")]
fn as_electrical_charge(name: &str, value: &str) -> Result<ElectricCharge, Box<dyn Error>> {
    let result = value.parse();
    match result {
        Ok(value) => Ok(ElectricCharge::new::<milliampere_hour>(value)),
        Err(e) => Err(format!("Failed to parse ElectricCharge {}: {}. {}", name, value, e).into()),
    }
}

#[cfg(feature = "uom")]
fn as_volume(name: &str, value: &str, factor: Option<f32>) -> Result<Volume, Box<dyn Error>> {
    let result = value.parse();
    match result {
        Ok(value) => match factor {
            Some(factor) => Ok(Volume::new::<liter>(value * factor)),
            None => Ok(Volume::new::<liter>(value)),
        },
        Err(e) => Err(format!("Failed to parse Volume {}: {}. {}", name, value, e).into()),
    }
}

fn as_bool(name: &str, value: &str) -> Result<bool, Box<dyn Error>> {
    let value = value
        .parse::<bool>()
        .map_err(|e| format!("Failed to parse bool {}: {}. {}", name, value, e))?;
    Ok(value)
}

#[cfg(test)]
mod tests;
