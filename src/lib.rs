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

use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;

use serde::Deserialize;
use serde::Serialize;
use thiserror::Error;

/// Errors that can occur when interacting with the RealFlight simulator.
#[derive(Debug, Error)]
pub enum BridgeError {
    /// Connection to the simulator failed
    #[error("Connection failed: {0}")]
    Connection(#[from] std::io::Error),

    /// Initialization failed
    #[error("Initialization failed: {0}")]
    Initialization(String),

    /// SOAP fault returned by the simulator
    #[error("SOAP fault: {0}")]
    SoapFault(String),

    /// Failed to parse simulator response
    #[error("Parse error for field '{field}': {message}")]
    Parse { field: String, message: String },
}

use std::time::Duration;
use std::time::Instant;

/// Conditional type aliases for physical quantities.
/// With `uom` feature: strongly-typed SI units
/// Without `uom` feature: raw f32 values
#[cfg(feature = "uom")]
mod unit_types {
    pub use uom::si::f32::{
        Acceleration, Angle, AngularVelocity, ElectricCharge, ElectricCurrent, ElectricPotential,
        Length, Time, Velocity, Volume,
    };
}

#[cfg(not(feature = "uom"))]
mod unit_types {
    pub type Velocity = f32;
    pub type Length = f32;
    pub type AngularVelocity = f32;
    pub type Angle = f32;
    pub type Acceleration = f32;
    pub type ElectricPotential = f32;
    pub type ElectricCurrent = f32;
    pub type ElectricCharge = f32;
    pub type Volume = f32;
    pub type Time = f32;
}

use unit_types::*;

#[cfg(any(test, feature = "bench-internals"))]
pub use decoders::decode_simulator_state;

#[cfg(any(test, feature = "bench-internals"))]
pub use decoders::extract_element;

pub mod bridge;
mod decoders;

mod soap_client;

/// Default RealFlight simulator address (localhost on standard port)
pub const DEFAULT_SIMULATOR_HOST: &str = "127.0.0.1:18083";

#[doc(inline)]
pub use bridge::RealFlightBridge;
#[doc(inline)]
pub use bridge::local::Configuration;
#[doc(inline)]
pub use bridge::local::RealFlightLocalBridge;
#[doc(inline)]
pub use bridge::proxy::ProxyServer;
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
pub use bridge::proxy::AsyncProxyServer;
#[cfg(feature = "rt-tokio")]
#[doc(inline)]
pub use bridge::remote::{AsyncRemoteBridge, AsyncRemoteBridgeBuilder};

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

/// Represents the complete state of the simulated aircraft in RealFlight.
/// Physical quantities use metric units (strongly-typed with `uom` feature, raw f32 otherwise).
#[derive(Default, Debug, Serialize, Deserialize, PartialEq)]
pub struct SimulatorState {
    /// Previous control inputs that led to this state
    pub previous_inputs: ControlInputs,
    /// Velocity relative to the air mass [meters/second]
    pub airspeed: Velocity,
    /// Altitude above sea level [meters]
    pub altitude_asl: Length,
    /// Altitude above ground level [meters]
    pub altitude_agl: Length,
    /// Velocity relative to the ground [meters/second]
    pub groundspeed: Velocity,
    /// Pitch rate around body Y axis [degrees/second]
    pub pitch_rate: AngularVelocity,
    /// Roll rate around body X axis [degrees/second]
    pub roll_rate: AngularVelocity,
    /// Yaw rate around body Z axis [degrees/second]
    pub yaw_rate: AngularVelocity,
    /// Heading angle (true north reference) [degrees]
    pub azimuth: Angle,
    /// Pitch angle (nose up reference) [degrees]
    pub inclination: Angle,
    /// Roll angle (right wing down reference) [degrees]
    pub roll: Angle,
    /// Aircraft position along world X axis (North) [meters]
    pub aircraft_position_x: Length,
    /// Aircraft position along world Y axis (East) [meters]
    pub aircraft_position_y: Length,
    /// Velocity component along world X axis (North) [meters/second]
    pub velocity_world_u: Velocity,
    /// Velocity component along world Y axis (East) [meters/second]
    pub velocity_world_v: Velocity,
    /// Velocity component along world Z axis (Down) [meters/second]
    pub velocity_world_w: Velocity,
    /// Forward velocity in body frame [meters/second]
    pub velocity_body_u: Velocity,
    /// Lateral velocity in body frame [meters/second]
    pub velocity_body_v: Velocity,
    /// Vertical velocity in body frame [meters/second]
    pub velocity_body_w: Velocity,
    /// Acceleration along world X axis (North) [meters/second²]
    pub acceleration_world_ax: Acceleration,
    /// Acceleration along world Y axis (East) [meters/second²]
    pub acceleration_world_ay: Acceleration,
    /// Acceleration along world Z axis (Down) [meters/second²]
    pub acceleration_world_az: Acceleration,
    /// Acceleration along body X axis (Forward) [meters/second²]
    pub acceleration_body_ax: Acceleration,
    /// Acceleration along body Y axis (Right) [meters/second²]
    pub acceleration_body_ay: Acceleration,
    /// Acceleration along body Z axis (Down) [meters/second²]
    pub acceleration_body_az: Acceleration,
    /// Wind velocity along world X axis [meters/second]
    pub wind_x: Velocity,
    /// Wind velocity along world Y axis [meters/second]
    pub wind_y: Velocity,
    /// Wind velocity along world Z axis [meters/second]
    pub wind_z: Velocity,
    /// Propeller RPM for piston/electric aircraft [revolutions/minute]
    pub prop_rpm: f32,
    /// Main rotor RPM for helicopters [revolutions/minute]
    pub heli_main_rotor_rpm: f32,
    /// Battery voltage [volts]
    pub battery_voltage: ElectricPotential,
    /// Current draw from battery [amperes]
    pub battery_current_draw: ElectricCurrent,
    /// Remaining battery capacity [milliamperes-hour]
    pub battery_remaining_capacity: ElectricCharge,
    /// Remaining fuel volume [ounces]
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
/// - `frequency`: An approximate request rate, calculated as `(request_count / runtime)`.
/// - `request_count`: The total number of SOAP requests sent to the simulator. Loops back to 0 after `u32::MAX`.
///
/// ```no_run
/// use realflight_bridge::{RealFlightLocalBridge, BridgeError};
///
/// fn main() -> Result<(), BridgeError> {
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

#[cfg(test)]
pub mod tests;

#[cfg(test)]
mod statistics_tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn new_starts_with_zero_counts() {
        let engine = StatisticsEngine::new();
        let snapshot = engine.snapshot();

        assert_eq!(snapshot.request_count, 0);
        assert_eq!(snapshot.error_count, 0);
    }

    #[test]
    fn increment_request_count_increases_count() {
        let engine = StatisticsEngine::new();

        engine.increment_request_count();
        engine.increment_request_count();
        engine.increment_request_count();

        assert_eq!(engine.snapshot().request_count, 3);
    }

    #[test]
    fn increment_error_count_increases_count() {
        let engine = StatisticsEngine::new();

        engine.increment_error_count();
        engine.increment_error_count();

        assert_eq!(engine.snapshot().error_count, 2);
    }

    #[test]
    fn runtime_increases_over_time() {
        let engine = StatisticsEngine::new();

        thread::sleep(Duration::from_millis(10));

        let snapshot = engine.snapshot();
        assert!(snapshot.runtime >= Duration::from_millis(10));
    }

    #[test]
    fn frequency_calculated_correctly() {
        let engine = StatisticsEngine::new();

        // Wait a bit then add requests
        thread::sleep(Duration::from_millis(50));
        engine.increment_request_count();
        engine.increment_request_count();

        let snapshot = engine.snapshot();
        // Frequency should be roughly 2 / 0.05 = 40, but allow wide margin
        assert!(snapshot.frequency > 0.0);
        assert!(snapshot.frequency < 100.0);
    }

    #[test]
    fn default_creates_new_engine() {
        let engine = StatisticsEngine::default();
        let snapshot = engine.snapshot();

        assert_eq!(snapshot.request_count, 0);
        assert_eq!(snapshot.error_count, 0);
    }
}
