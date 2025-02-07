use std::error::Error;

use uom::si::angular_velocity::degree_per_second;
use uom::si::f64::*;
use uom::si::length::kilometer;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;
use uom::si::angle::degree;

use super::{SimulatorState, extract_element};


pub fn decode_simulator_state(xml: &str) -> Result<SimulatorState, Box<dyn Error>> {
    let mut state = SimulatorState::default();

    let current_physics_time = extract_element( "m-currentPhysicsTime-SEC", xml).expect("currentPhysicsTime not found");
    let current_physics_time: f64 = current_physics_time.parse().unwrap();
    state.current_physics_time = Time::new::<second>(current_physics_time);

    let current_speed_multiplier = extract_element("m-currentPhysicsSpeedMultiplier", xml).expect("currentPhysicsSpeedMultiplier not found");
    let current_speed_multiplier: f64 = current_speed_multiplier.parse().unwrap();
    state.current_physics_speed_multiplier = current_speed_multiplier;

    Ok(state)
}
