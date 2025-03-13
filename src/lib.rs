//! [![github]](https://github.com/wboayue/realflight-link)&ensp;[![crates-io]](https://crates.io/crates/realflight-link)&ensp;[![license]](https://opensource.org/licenses/MIT)
//!
//! [github]: https://img.shields.io/badge/github-8da0cb?style=for-the-badge&labelColor=555555&logo=github
//! [crates-io]: https://img.shields.io/badge/crates.io-fc8d62?style=for-the-badge&labelColor=555555&logo=rust
//! [license]: https://img.shields.io/badge/License-MIT-blue.svg?style=for-the-badge&labelColor=555555
//!
//! RealFlight is a leading RC flight simulator that provides a realistic, physics-based environment for flying fixed-wing aircraft, helicopters, and drones. Used by both hobbyists and professionals, it simulates aerodynamics, wind conditions, and control responses, making it an excellent tool for flight control algorithm validation.
//!
//! RealFlightBridge is a Rust library that interfaces with RealFlight Link, enabling external flight controllers to interact with the simulator. It allows developers to:
//!
//! * Send control commands to simulated aircraft.
//! * Receive real-time simulated flight data for state estimation and control.
//! * Test stabilization and autonomy algorithms in a controlled environment.
//!
//! See [README](https://github.com/wboayue/realflight-link) for examples and usage.

use std::error::Error;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;

use serde::Deserialize;
use serde::Serialize;
use soap_client::tcp::TcpSoapClient;
use std::time::Duration;
use std::time::Instant;
use uom::si::f32::*;

#[cfg(test)]
use soap_client::stub::StubSoapClient;

#[cfg(any(test, feature = "bench-internals"))]
pub use decoders::decode_simulator_state;

#[cfg(not(any(test, feature = "bench-internals")))]
use decoders::extract_element;

#[cfg(any(test, feature = "bench-internals"))]
pub use decoders::extract_element;

pub mod bridge;
mod decoders;
mod soap_client;

#[doc(inline)]
pub use bridge::local::Configuration;
#[doc(inline)]
pub use bridge::local::RealFlightLocalBridge;
#[doc(inline)]
pub use bridge::remote::ProxyServer;
#[doc(inline)]
pub use bridge::remote::RealFlightRemoteBridge;

#[derive(Debug)]
struct SoapResponse {
    status_code: u32,
    body: String,
}

impl From<SoapResponse> for Result<(), Box<dyn Error>> {
    fn from(val: SoapResponse) -> Self {
        match val.status_code {
            200 => Ok(()),
            _ => Err(decode_fault(&val).into()),
        }
    }
}

/// Control inputs for the RealFlight simulator using the standard RC channel mapping.
/// Each channel value should be between 0.0 (minimum) and 1.0 (maximum).
///
/// # Standard RC Channel Mapping
///
/// The 12 available channels typically map to the following controls:
///
/// * Channel 1 (Aileron): Controls roll movement
///   - 0.0: Full left roll
///   - 0.5: Neutral
///   - 1.0: Full right roll
///
/// * Channel 2 (Elevator): Controls pitch movement
///   - 0.0: Full down pitch (nose down)
///   - 0.5: Neutral
///   - 1.0: Full up pitch (nose up)
///
/// * Channel 3 (Throttle): Controls engine power
///   - 0.0: Zero throttle (engine off/idle)
///   - 1.0: Full throttle
///
/// * Channel 4 (Rudder): Controls yaw movement
///   - 0.0: Full left yaw
///   - 0.5: Neutral
///   - 1.0: Full right yaw
///
/// * Channel 5: Commonly used for flight modes
///   - Often used as a 3-position switch (0.0, 0.5, 1.0)
///   - Typical modes: Manual, Stabilized, Auto
///
/// * Channel 6: Commonly used for collective pitch (helicopters)
///   - 0.0: Full negative pitch
///   - 0.5: Zero pitch
///   - 1.0: Full positive pitch
///
/// * Channels 7-12: Auxiliary channels
///   - Can be mapped to various functions like:
///     - Flaps
///     - Landing gear
///     - Camera gimbal
///     - Lights
///     - Custom functions#[derive(Default, Debug)]
#[derive(Default, Debug, Serialize, Deserialize, Clone)]
pub struct ControlInputs {
    /// Array of 12 channel values, each between 0.0 and 1.0
    pub channels: [f32; 12],
}

/// Represents the complete state of the simulated aircraft in RealFlight.
/// All physical quantities use SI units through the `uom` crate.
#[derive(Default, Debug, Serialize, Deserialize)]
pub struct SimulatorState {
    /// Previous control inputs that led to this state
    pub previous_inputs: ControlInputs,
    /// Velocity relative to the air mass
    pub airspeed: Velocity,
    /// Altitude above sea level
    pub altitude_asl: Length,
    /// Altitude above ground level
    pub altitude_agl: Length,
    /// Velocity relative to the ground
    pub groundspeed: Velocity,
    /// Pitch rate around body Y axis
    pub pitch_rate: AngularVelocity,
    /// Roll rate around body X axis
    pub roll_rate: AngularVelocity,
    /// Yaw rate around body Z axis
    pub yaw_rate: AngularVelocity,
    /// Heading angle (true north reference)
    pub azimuth: Angle,
    /// Pitch angle (nose up reference)
    pub inclination: Angle,
    /// Roll angle (right wing down reference)
    pub roll: Angle,
    /// Aircraft position along world X axis (North)
    pub aircraft_position_x: Length,
    /// Aircraft position along world Y axis (East)
    pub aircraft_position_y: Length,
    /// Velocity component along world X axis (North)
    pub velocity_world_u: Velocity,
    /// Velocity component along world Y axis (East)
    pub velocity_world_v: Velocity,
    /// Velocity component along world Z axis (Down)
    pub velocity_world_w: Velocity,
    /// Forward velocity in body frame
    pub velocity_body_u: Velocity,
    // Lateral velocity in body frame
    pub velocity_body_v: Velocity,
    // Vertical velocity in body frame
    pub velocity_body_w: Velocity,
    // Acceleration along world X axis (North)
    pub acceleration_world_ax: Acceleration,
    // Acceleration along world Y axis (East)
    pub acceleration_world_ay: Acceleration,
    // Acceleration along world Z axis (Down)
    pub acceleration_world_az: Acceleration,
    // Acceleration along body X axis (Forward)
    pub acceleration_body_ax: Acceleration,
    // Acceleration along body Y axis (Right)
    pub acceleration_body_ay: Acceleration,
    /// Acceleration along body Z axis (Down)
    pub acceleration_body_az: Acceleration,
    /// Wind velocity along world X axis
    pub wind_x: Velocity,
    /// Wind velocity along world Y axis
    pub wind_y: Velocity,
    /// Wind velocity along world Z axis
    pub wind_z: Velocity,
    /// Propeller RPM for piston/electric aircraft
    pub prop_rpm: f32,
    /// Main rotor RPM for helicopters
    pub heli_main_rotor_rpm: f32,
    /// Battery voltage
    pub battery_voltage: ElectricPotential,
    /// Current draw from battery
    pub battery_current_draw: ElectricCurrent,
    /// Remaining battery capacity
    pub battery_remaining_capacity: ElectricCharge,
    /// Remaining fuel volume
    pub fuel_remaining: Volume,
    /// True if aircraft is in a frozen/paused state
    pub is_locked: bool,
    /// True if aircraft has lost components due to damage
    pub has_lost_components: bool,
    /// True if any engine is currently running
    pub an_engine_is_running: bool,
    /// True if aircraft is in contact with ground
    pub is_touching_ground: bool,
    /// Current status message from simulator
    pub current_aircraft_status: String,
    /// Current simulation time
    pub current_physics_time: Time,
    /// Current time acceleration factor
    pub current_physics_speed_multiplier: f32,
    /// Quaternion X component (scalar)
    pub orientation_quaternion_x: f32,
    /// Quaternion Y component (scalar)
    pub orientation_quaternion_y: f32,
    /// Quaternion Z component (scalar)
    pub orientation_quaternion_z: f32,
    /// Quaternion W component (scalar)
    pub orientation_quaternion_w: f32,
    /// True if external flight controller is active
    pub flight_axis_controller_is_active: bool,
    /// True if reset button was pressed
    pub reset_button_has_been_pressed: bool,
}

/// Represents a snapshot of performance metrics for a running `RealFlightBridge`.
///
/// The `Statistics` struct is returned by [`RealFlightBridge::statistics`](crate::RealFlightBridge::statistics)
/// and captures various counters and timings that can help diagnose performance issues
/// or monitor real-time operation.
///
/// # Fields
///
/// - `runtime`: The total elapsed time since the `RealFlightBridge` instance was created.
/// - `error_count`: The number of errors (e.g., connection errors, SOAP faults) encountered so far.
/// - `frame_rate`: An approximate request rate, calculated as `(request_count / runtime)`.
/// - `request_count`: The total number of SOAP requests sent to the simulator. Loops back to 0 after `u32::MAX`.
///
/// ```no_run
/// use realflight_bridge::{RealFlightLocalBridge, Configuration};
/// use std::error::Error;
///
/// fn main() -> Result<(), Box<dyn Error>> {
///     let bridge = RealFlightLocalBridge::new()?;
///
///     // Send some commands...
///
///     // Now retrieve statistics to assess performance
///     let stats = bridge.statistics();
///     println!("Runtime: {:?}", stats.runtime);
///     println!("Frequency: {:.2} Hz", stats.frequency);
///     println!("Errors so far: {}", stats.error_count);
///
///     Ok(())
/// }
/// ```
///
/// This information can help identify connection bottlenecks, excessive errors,
/// or confirm that a high-frequency control loop is operating as expected.
#[derive(Debug)]
pub struct Statistics {
    pub runtime: Duration,
    pub error_count: u32,
    pub frequency: f32,
    pub request_count: u32,
}

/// Statistics for the RealFlightBridge
pub(crate) struct StatisticsEngine {
    start_time: Instant,
    error_count: AtomicU32,
    request_count: AtomicU32,
}

impl StatisticsEngine {
    pub fn new() -> Self {
        StatisticsEngine {
            start_time: Instant::now(),
            error_count: AtomicU32::new(0),
            request_count: AtomicU32::new(0),
        }
    }

    pub fn snapshot(&self) -> Statistics {
        Statistics {
            runtime: self.start_time.elapsed(),
            error_count: self.error_count(),
            frequency: self.frame_rate(),
            request_count: self.request_count(),
        }
    }

    fn error_count(&self) -> u32 {
        self.error_count.load(Ordering::Relaxed)
    }

    fn request_count(&self) -> u32 {
        self.request_count.load(Ordering::Relaxed)
    }

    fn increment_request_count(&self) {
        self.request_count.fetch_add(1, Ordering::Relaxed);
    }

    fn increment_error_count(&self) {
        self.error_count.fetch_add(1, Ordering::Relaxed);
    }

    fn frame_rate(&self) -> f32 {
        self.request_count() as f32 / self.start_time.elapsed().as_secs_f32()
    }
}

impl Default for StatisticsEngine {
    fn default() -> Self {
        Self::new()
    }
}

fn decode_fault(response: &SoapResponse) -> String {
    match extract_element("detail", &response.body) {
        Some(message) => message,
        None => "Failed to extract error message".into(),
    }
}

#[cfg(test)]
pub mod tests;
