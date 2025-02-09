use core::error;
use std::{collections::BTreeMap, error::Error};

use uom::si::angular_velocity::degree_per_second;
use uom::si::f64::*;
use uom::si::length::kilometer;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;
use uom::si::{angle::degree, length::meter};

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

pub fn decode_simulator_state(xml: &str) -> Result<SimulatorState, Box<dyn Error>> {
    let mut state = SimulatorState::default();

    let raw = extract_elements(xml);

    state.current_physics_time = as_time(&raw, "m-currentPhysicsTime-SEC")?;
    state.current_physics_speed_multiplier = as_double(&raw, "m-currentPhysicsSpeedMultiplier")?;
    state.airspeed = as_velocity(&raw, "m-airspeed-MPS")?;
    state.altitude_asl = as_length(&raw, "m-altitudeASL-MTR")?;
    state.altitude_agl = as_length(&raw, "m-altitudeAGL-MTR")?;
    state.groundspeed = as_velocity(&raw, "m-groundspeed-MPS")?;

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

fn as_length(state: &BTreeMap<String, String>, field: &str) -> Result<Length, Box<dyn Error>> {
    let value = state.get(field).ok_or(format!("{} not found", field))?;
    let value: f64 = value.parse()?;
    Ok(Length::new::<meter>(value))
}
