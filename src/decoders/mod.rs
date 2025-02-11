use std::{collections::BTreeMap, error::Error};

use uom::si::acceleration::meter_per_second_squared;
use uom::si::angle::degree;
use uom::si::electric_charge::milliampere_hour;
use uom::si::electric_current::ampere;
use uom::si::electric_potential::volt;
use uom::si::length::meter;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;
use uom::si::volume::liter;
use uom::si::{angular_velocity::degree_per_second, f64::*};

use super::{extract_element, SimulatorState};

const STATE_FIELDS: [&str; 45] = [
    "m-currentPhysicsTime-SEC",
    "m-currentPhysicsSpeedMultiplier",
    "m-airspeed-MPS",
    "m-altitudeASL-MTR",
    "m-altitudeAGL-MTR",
    "m-groundspeed-MPS",
    "m-pitchRate-DEGpSEC",
    "m-rollRate-DEGpSEC",
    "m-yawRate-DEGpSEC",
    "m-azimuth-DEG",
    "m-inclination-DEG",
    "m-roll-DEG",
    "m-orientationQuaternion-X",
    "m-orientationQuaternion-Y",
    "m-orientationQuaternion-Z",
    "m-orientationQuaternion-W",
    "m-aircraftPositionX-MTR",
    "m-aircraftPositionY-MTR",
    "m-velocityWorldU-MPS",
    "m-velocityWorldV-MPS",
    "m-velocityWorldW-MPS",
    "m-velocityBodyU-MPS",
    "m-velocityBodyV-MPS",
    "m-velocityBodyW-MPS",
    "m-accelerationWorldAX-MPS2",
    "m-accelerationWorldAY-MPS2",
    "m-accelerationWorldAZ-MPS2",
    "m-accelerationBodyAX-MPS2",
    "m-accelerationBodyAY-MPS2",
    "m-accelerationBodyAZ-MPS2",
    "m-windX-MPS",
    "m-windY-MPS",
    "m-windZ-MPS",
    "m-propRPM",
    "m-heliMainRotorRPM",
    "m-batteryVoltage-VOLTS",
    "m-batteryCurrentDraw-AMPS",
    "m-batteryRemainingCapacity-MAH",
    "m-fuelRemaining-OZ",
    "m-isLocked",
    "m-hasLostComponents",
    "m-anEngineIsRunning",
    "m-isTouchingGround",
    "m-flightAxisControllerIsActive",
    "m-currentAircraftStatus",
];

pub const OUNCES_PER_LITER: f64 = 33.814;

pub fn decode_simulator_state(xml: &str) -> Result<SimulatorState, Box<dyn Error>> {
    let mut state = SimulatorState::default();

    let raw = extract_elements(xml);

    state.current_physics_time = as_time(&raw, "m-currentPhysicsTime-SEC")?;
    state.current_physics_speed_multiplier = as_double(&raw, "m-currentPhysicsSpeedMultiplier")?;
    state.airspeed = as_velocity(&raw, "m-airspeed-MPS")?;
    state.altitude_asl = as_length(&raw, "m-altitudeASL-MTR")?;
    state.altitude_agl = as_length(&raw, "m-altitudeAGL-MTR")?;
    state.groundspeed = as_velocity(&raw, "m-groundspeed-MPS")?;
    state.pitch_rate = as_angular_velocity(&raw, "m-pitchRate-DEGpSEC")?;
    state.roll_rate = as_angular_velocity(&raw, "m-rollRate-DEGpSEC")?;
    state.yaw_rate = as_angular_velocity(&raw, "m-yawRate-DEGpSEC")?;
    state.azimuth = as_angle(&raw, "m-azimuth-DEG")?;
    state.inclination = as_angle(&raw, "m-inclination-DEG")?;
    state.roll = as_angle(&raw, "m-roll-DEG")?;
    state.orientation_quaternion_x = as_double(&raw, "m-orientationQuaternion-X")?;
    state.orientation_quaternion_y = as_double(&raw, "m-orientationQuaternion-Y")?;
    state.orientation_quaternion_z = as_double(&raw, "m-orientationQuaternion-Z")?;
    state.orientation_quaternion_w = as_double(&raw, "m-orientationQuaternion-W")?;
    state.aircraft_position_x = as_length(&raw, "m-aircraftPositionX-MTR")?;
    state.aircraft_position_y = as_length(&raw, "m-aircraftPositionY-MTR")?;
    state.velocity_world_u = as_velocity(&raw, "m-velocityWorldU-MPS")?;
    state.velocity_world_v = as_velocity(&raw, "m-velocityWorldV-MPS")?;
    state.velocity_world_w = as_velocity(&raw, "m-velocityWorldW-MPS")?;
    state.velocity_body_u = as_velocity(&raw, "m-velocityBodyU-MPS")?;
    state.velocity_body_v = as_velocity(&raw, "m-velocityBodyV-MPS")?;
    state.velocity_body_w = as_velocity(&raw, "m-velocityBodyW-MPS")?;
    state.acceleration_world_ax = as_acceleration(&raw, "m-accelerationWorldAX-MPS2")?;
    state.acceleration_world_ay = as_acceleration(&raw, "m-accelerationWorldAY-MPS2")?;
    state.acceleration_world_az = as_acceleration(&raw, "m-accelerationWorldAZ-MPS2")?;
    state.acceleration_body_ax = as_acceleration(&raw, "m-accelerationBodyAX-MPS2")?;
    state.acceleration_body_ay = as_acceleration(&raw, "m-accelerationBodyAY-MPS2")?;
    state.acceleration_body_az = as_acceleration(&raw, "m-accelerationBodyAZ-MPS2")?;
    state.wind_x = as_velocity(&raw, "m-windX-MPS")?;
    state.wind_y = as_velocity(&raw, "m-windY-MPS")?;
    state.wind_z = as_velocity(&raw, "m-windZ-MPS")?;
    state.prop_rpm = as_double(&raw, "m-propRPM")?;
    state.heli_main_rotor_rpm = as_double(&raw, "m-heliMainRotorRPM")?;
    state.battery_voltage = as_electrical_potential(&raw, "m-batteryVoltage-VOLTS")?;
    state.battery_current_draw = as_electrical_current(&raw, "m-batteryCurrentDraw-AMPS")?;
    state.battery_remaining_capacity =
        as_electrical_charge(&raw, "m-batteryRemainingCapacity-MAH")?;
    state.fuel_remaining = as_volume(&raw, "m-fuelRemaining-OZ", Some(1.0 / OUNCES_PER_LITER))?;
    state.is_locked = as_bool(&raw, "m-isLocked")?;
    state.has_lost_components = as_bool(&raw, "m-hasLostComponents")?;
    state.an_engine_is_running = as_bool(&raw, "m-anEngineIsRunning")?;
    state.is_touching_ground = as_bool(&raw, "m-isTouchingGround")?;
    state.flight_axis_controller_is_active = as_bool(&raw, "m-flightAxisControllerIsActive")?;
    state.current_aircraft_status = as_string(&raw, "m-currentAircraftStatus")?;

    Ok(state)
}

fn extract_elements(xml: &str) -> BTreeMap<String, String> {
    let mut elements = BTreeMap::new();
    for field in STATE_FIELDS.iter() {
        if let Some(value) = extract_element(field, xml) {
            elements.entry(field.to_string()).or_insert(value);
        }
    }
    elements
}

fn as_time(state: &BTreeMap<String, String>, field: &str) -> Result<Time, Box<dyn Error>> {
    let value = state.get(field).ok_or(format!("{} not found", field))?;
    let value: f64 = value.parse()?;
    Ok(Time::new::<second>(value))
}

fn as_double(state: &BTreeMap<String, String>, field: &str) -> Result<f64, Box<dyn Error>> {
    let value = state.get(field).ok_or(format!("{} not found", field))?;
    let value: f64 = value.parse()?;
    Ok(value)
}

fn as_velocity(state: &BTreeMap<String, String>, field: &str) -> Result<Velocity, Box<dyn Error>> {
    let value = state.get(field).ok_or(format!("{} not found", field))?;
    let value: f64 = value.parse()?;
    Ok(Velocity::new::<meter_per_second>(value))
}

fn as_angular_velocity(
    state: &BTreeMap<String, String>,
    field: &str,
) -> Result<AngularVelocity, Box<dyn Error>> {
    let value = state.get(field).ok_or(format!("{} not found", field))?;
    let value: f64 = value.parse()?;
    Ok(AngularVelocity::new::<degree_per_second>(value))
}

fn as_length(state: &BTreeMap<String, String>, field: &str) -> Result<Length, Box<dyn Error>> {
    let value = state.get(field).ok_or(format!("{} not found", field))?;
    let value: f64 = value.parse()?;
    Ok(Length::new::<meter>(value))
}

fn as_angle(state: &BTreeMap<String, String>, field: &str) -> Result<Angle, Box<dyn Error>> {
    let value = state.get(field).ok_or(format!("{} not found", field))?;
    let value: f64 = value.parse()?;
    Ok(Angle::new::<degree>(value))
}

fn as_acceleration(
    state: &BTreeMap<String, String>,
    field: &str,
) -> Result<Acceleration, Box<dyn Error>> {
    let value = state.get(field).ok_or(format!("{} not found", field))?;
    let value: f64 = value.parse()?;
    Ok(Acceleration::new::<meter_per_second_squared>(value))
}

fn as_electrical_potential(
    state: &BTreeMap<String, String>,
    field: &str,
) -> Result<ElectricPotential, Box<dyn Error>> {
    let value = state.get(field).ok_or(format!("{} not found", field))?;
    let value: f64 = value.parse()?;
    Ok(ElectricPotential::new::<volt>(value))
}

fn as_electrical_current(
    state: &BTreeMap<String, String>,
    field: &str,
) -> Result<ElectricCurrent, Box<dyn Error>> {
    let value = state.get(field).ok_or(format!("{} not found", field))?;
    let value: f64 = value.parse()?;
    Ok(ElectricCurrent::new::<ampere>(value))
}

fn as_electrical_charge(
    state: &BTreeMap<String, String>,
    field: &str,
) -> Result<ElectricCharge, Box<dyn Error>> {
    let value = state.get(field).ok_or(format!("{} not found", field))?;
    let value: f64 = value.parse()?;
    Ok(ElectricCharge::new::<milliampere_hour>(value))
}

fn as_volume(
    state: &BTreeMap<String, String>,
    field: &str,
    scale: Option<f64>,
) -> Result<Volume, Box<dyn Error>> {
    let value = state.get(field).ok_or(format!("{} not found", field))?;
    let value: f64 = value.parse()?;
    let value = match scale {
        Some(scale) => value * scale,
        None => value,
    };
    Ok(Volume::new::<liter>(value))
}

fn as_bool(state: &BTreeMap<String, String>, field: &str) -> Result<bool, Box<dyn Error>> {
    let value = state.get(field).ok_or(format!("{} not found", field))?;
    Ok(value == "true")
}

fn as_string(state: &BTreeMap<String, String>, field: &str) -> Result<String, Box<dyn Error>> {
    let value = state.get(field).ok_or(format!("{} not found", field))?;
    Ok(value.to_string())
}
