//! [![github]](https://github.com/wboayue/realflight-bridge)&ensp;[![crates-io]](https://crates.io/crates/realflight-bridge)&ensp;[![license]](https://opensource.org/licenses/MIT)
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
//! See [README](https://github.com/wboayue/realflight-bridge) for examples and usage.

use serde::Deserialize;
use serde::Serialize;
use thiserror::Error;

/// Errors that can occur when interacting with the RealFlight simulator.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum BridgeError {
    /// Connection to the simulator failed
    #[error("Connection failed: {0}")]
    Connection(#[from] std::io::Error),

    /// Initialization failed
    #[error("Initialization failed: {0}")]
    Initialization(String),

    /// SOAP fault returned by the simulator (directly, or relayed by the proxy)
    #[error("SOAP fault: {0}")]
    SoapFault(String),

    /// Malformed or unexpected response from the simulator or proxy
    #[error("Protocol error: {0}")]
    Protocol(String),

    /// Failed to parse simulator response
    #[error("Parse error for field '{field}': {message}")]
    Parse { field: String, message: String },
}
#[cfg(any(test, feature = "bench-internals"))]
pub use decoders::decode_simulator_state;

#[cfg(any(test, feature = "bench-internals"))]
pub use decoders::extract_element;

#[cfg(any(test, feature = "bench-internals"))]
pub use encoders::encode_control_inputs;

pub mod bridge;
mod decoders;
mod defaults;
mod encoders;
mod soap_client;
mod statistics;

pub use statistics::Statistics;
pub(crate) use statistics::StatisticsEngine;

/// Default RealFlight simulator address (localhost on standard port)
pub const DEFAULT_SIMULATOR_HOST: &str = "127.0.0.1:18083";

#[doc(inline)]
pub use bridge::RealFlightBridge;
#[doc(inline)]
pub use bridge::local::Configuration;
#[doc(inline)]
pub use bridge::local::RealFlightLocalBridge;
#[doc(inline)]
pub use bridge::remote::RealFlightRemoteBridge;

// Async exports (requires rt-tokio feature)
#[cfg(feature = "rt-tokio")]
#[doc(inline)]
pub use bridge::AsyncBridge;
#[cfg(feature = "rt-tokio")]
#[doc(inline)]
pub use bridge::local::{AsyncLocalBridge, AsyncLocalBridgeBuilder};
#[cfg(feature = "rt-tokio")]
#[doc(inline)]
pub use bridge::remote::{AsyncRemoteBridge, AsyncRemoteBridgeBuilder};

// Re-export for binary (not part of public API)
#[cfg(feature = "rt-tokio")]
#[doc(hidden)]
pub use bridge::proxy::AsyncProxyServer;

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
///     - Custom functions
#[derive(Default, Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ControlInputs {
    /// Array of 12 channel values, each between 0.0 and 1.0
    pub channels: [f32; 12],
}

/// Three-component vector. Frame and unit depend on the field holding it; see
/// [`SimulatorState`].
///
/// Converts to and from `[x, y, z]`.
#[derive(Default, Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Vector3 {
    /// X component (RealFlight U for velocities)
    pub x: f32,
    /// Y component (RealFlight V for velocities)
    pub y: f32,
    /// Z component (RealFlight W for velocities)
    pub z: f32,
}

impl From<[f32; 3]> for Vector3 {
    fn from([x, y, z]: [f32; 3]) -> Self {
        Self { x, y, z }
    }
}

impl From<Vector3> for [f32; 3] {
    fn from(v: Vector3) -> Self {
        [v.x, v.y, v.z]
    }
}

/// Orientation quaternion in RealFlight's convention (unitless).
///
/// Converts to and from `[x, y, z, w]` (scalar last). RealFlight's axes differ
/// from NED; see [`SimulatorState::orientation`].
#[derive(Default, Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Quaternion {
    /// X (i) component
    pub x: f32,
    /// Y (j) component
    pub y: f32,
    /// Z (k) component
    pub z: f32,
    /// W (scalar) component
    pub w: f32,
}

impl From<[f32; 4]> for Quaternion {
    fn from([x, y, z, w]: [f32; 4]) -> Self {
        Self { x, y, z, w }
    }
}

impl From<Quaternion> for [f32; 4] {
    fn from(q: Quaternion) -> Self {
        [q.x, q.y, q.z, q.w]
    }
}

/// Represents the complete state of the simulated aircraft in RealFlight.
///
/// # Units
///
/// Values are passed through unconverted from RealFlight. Field names carry a unit
/// suffix: `_m` meters, `_mps` m/s, `_mps2` m/s², `_deg` degrees, `_dps` deg/s,
/// `_v` volts, `_a` amps, `_mah` milliamp-hours, `_oz` US fluid ounces, `_s` seconds.
///
/// # Frames
///
/// RealFlight does not document its axes, and they are not consistent across
/// fields. Values are passed through as-is. The conventions below follow
/// ArduPilot's RealFlight SITL integration (`SIM_FlightAxis.cpp`):
///
/// * `velocity_world_mps`: x north, y east, z down (NED)
/// * `aircraft_position_x_m`, `aircraft_position_y_m`, `wind_mps`: x east, y north
///   (wind z down)
/// * `acceleration_body_mps2`: x forward, y right, z down
/// * `yaw_rate_dps`: positive is nose left, opposite to NED
/// * `orientation`: see field docs
///
/// `velocity_body_mps` and `acceleration_world_mps2` are not used by ArduPilot;
/// their axes are unverified.
#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SimulatorState {
    /// Previous control inputs that led to this state
    pub previous_inputs: ControlInputs,
    /// Velocity relative to the air mass (m/s)
    pub airspeed_mps: f32,
    /// Altitude above sea level (m)
    pub altitude_asl_m: f32,
    /// Altitude above ground level (m)
    pub altitude_agl_m: f32,
    /// Velocity relative to the ground (m/s)
    pub groundspeed_mps: f32,
    /// Pitch rate around body Y axis (deg/s)
    pub pitch_rate_dps: f32,
    /// Roll rate around body X axis (deg/s)
    pub roll_rate_dps: f32,
    /// Yaw rate (deg/s). Positive is nose left, opposite to NED
    pub yaw_rate_dps: f32,
    /// Heading angle (true north reference) (deg)
    pub azimuth_deg: f32,
    /// Pitch angle (nose up reference) (deg)
    pub inclination_deg: f32,
    /// Roll angle (right wing down reference) (deg)
    pub roll_deg: f32,
    /// Aircraft position, east (m)
    pub aircraft_position_x_m: f32,
    /// Aircraft position, north (m)
    pub aircraft_position_y_m: f32,
    /// Velocity in world frame, NED (m/s)
    pub velocity_world_mps: Vector3,
    /// Velocity in body frame (m/s). Axes unverified
    pub velocity_body_mps: Vector3,
    /// Acceleration in world frame (m/s²). Axes unverified
    pub acceleration_world_mps2: Vector3,
    /// Acceleration in body frame, forward/right/down (m/s²)
    pub acceleration_body_mps2: Vector3,
    /// Wind velocity: x east, y north, z down (m/s)
    pub wind_mps: Vector3,
    /// Propeller RPM for piston/electric aircraft (rpm)
    pub prop_rpm: f32,
    /// Main rotor RPM for helicopters (rpm)
    pub heli_main_rotor_rpm: f32,
    /// Battery voltage (V)
    pub battery_voltage_v: f32,
    /// Current draw from battery (A)
    pub battery_current_draw_a: f32,
    /// Remaining battery capacity (mAh)
    pub battery_remaining_capacity_mah: f32,
    /// Remaining fuel volume (US fl oz)
    pub fuel_remaining_oz: f32,
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
    /// Current simulation time (s)
    pub current_physics_time_s: f32,
    /// Current time acceleration factor (unitless)
    pub current_physics_speed_multiplier: f32,
    /// Aircraft orientation in RealFlight's convention.
    ///
    /// For a body-to-NED quaternion, NED `(w, x, y, z)` = RealFlight `(w, y, x, -z)`,
    /// as ArduPilot does.
    pub orientation: Quaternion,
    /// True if external flight controller is active
    pub flight_axis_controller_is_active: bool,
    /// True if reset button was pressed
    pub reset_button_has_been_pressed: bool,
}

#[cfg(test)]
mod tests;
