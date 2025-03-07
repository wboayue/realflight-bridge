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
use std::sync::Arc;

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

pub use bridge::remote::ProxyServer;
pub use bridge::remote::RealFlightRemoteBridge;

const UNUSED: &str = "";

/// A high-level client for interacting with RealFlight simulators via RealFlight Link.
///
/// # Overview
///
/// [RealFlightBridge] is your main entry point to controlling and querying the
/// RealFlight simulator. It exposes methods to:
///
/// - Send flight control inputs (e.g., RC channel data).
/// - Retrieve real-time flight state from the simulator.
/// - Toggle between internal and external RC control devices.
/// - Reset aircraft position and orientation.
///
/// # Examples
///
/// ```no_run
/// use realflight_bridge::{RealFlightBridge, Configuration, ControlInputs};
/// use std::error::Error;
///
/// fn main() -> Result<(), Box<dyn Error>> {
///     // Configure connection to the RealFlight simulator
///     let config = Configuration::default();
///
///     // Build a RealFlightBridge client
///     let bridge = RealFlightBridge::new(&config)?;
///
///     // Create sample control inputs
///     let mut inputs = ControlInputs::default();
///     inputs.channels[0] = 0.5; // Neutral aileron
///     inputs.channels[1] = 0.5; // Neutral elevator
///     inputs.channels[2] = 1.0; // Full throttle
///
///     // Enable external control
///     bridge.disable_rc()?;
///
///     // Exchange data with the simulator
///     let sim_state = bridge.exchange_data(&inputs)?;
///     println!("Current airspeed: {:?}", sim_state.airspeed);
///
///     // Return to internal control
///     bridge.enable_rc()?;
///
///     Ok(())
/// }
/// ```
///
/// # Error Handling
///
/// Methods that exchange data or mutate simulator state return `Result<T, Box<dyn Error>>`.
/// Common errors include:
///
/// - Connection timeouts
/// - SOAP faults (e.g., simulator not ready or invalid commands)
/// - Parsing issues for responses
///
/// Any non-2xx HTTP status code will typically return an error containing the simulator’s
/// fault message, if available.
///
/// # Statistics
///
/// Use [`statistics()`](#method.statistics) to retrieve current performance metrics
/// such as request count, errors, and average frame rate. This is useful for profiling
/// real-time loops or detecting dropped messages.
pub struct RealFlightBridge {
    statistics: Arc<StatisticsEngine>,
    soap_client: Box<dyn SoapClient>,
}

impl RealFlightBridge {
    /// Creates a new [RealFlightBridge] instance configured to communicate
    /// with a RealFlight simulator using a TCP-based Soap client.
    ///
    /// # Parameters
    ///
    /// - `configuration`: A [Configuration] specifying simulator address, connection
    ///   timeouts, and the number of pooled connections.
    ///
    /// # Returns
    ///
    /// A [Result] containing a fully initialized [RealFlightBridge] if the TCP connection
    /// pool is successfully created. Returns an error if the simulator address cannot be
    /// resolved or if the pool could not be initialized.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use realflight_bridge::{RealFlightBridge, Configuration};
    /// use std::error::Error;
    ///
    /// fn main() -> Result<(), Box<dyn Error>> {
    ///     // Use default localhost-based config.
    ///     let config = Configuration::default();
    ///
    ///     // Build a bridge to the RealFlight simulator.
    ///     let bridge = RealFlightBridge::new(&config)?;
    ///
    ///     // Now you can interact with RealFlight:
    ///     // - Send/receive flight control data
    ///     // - Reset aircraft
    ///     // - Toggle RC input
    ///
    ///     Ok(())
    /// }
    /// ```
    ///
    /// # Errors
    ///
    /// This function will return an error in the following situations:
    ///
    /// - If the simulator address specified in `configuration` is invalid.
    /// - If the TCP connection pool cannot be established (e.g., RealFlight is not running).
    pub fn new(configuration: &Configuration) -> Result<RealFlightBridge, Box<dyn Error>> {
        let statistics = Arc::new(StatisticsEngine::new());
        let soap_client = TcpSoapClient::new(configuration.clone(), statistics.clone())?;
        soap_client.ensure_pool_initialized()?;

        Ok(RealFlightBridge {
            statistics: statistics.clone(),
            soap_client: Box::new(soap_client),
        })
    }

    /// Creates a new RealFlightLink client
    /// simulator_url: the url to the RealFlight simulator
    #[cfg(test)]
    fn stub(mut soap_client: StubSoapClient) -> Result<RealFlightBridge, Box<dyn Error>> {
        let statistics = Arc::new(StatisticsEngine::new());

        soap_client.statistics = Some(statistics.clone());

        Ok(RealFlightBridge {
            statistics,
            soap_client: Box::new(soap_client),
        })
    }

    #[cfg(test)]
    fn requests(&self) -> Vec<String> {
        self.soap_client.requests().clone()
    }

    /// Get statistics for the RealFlightBridge
    pub fn statistics(&self) -> Statistics {
        self.statistics.snapshot()
    }

    /// Exchanges flight control data with the RealFlight simulator.
    ///
    /// This method transmits the provided [ControlInputs] (e.g., RC channel values)
    /// to the RealFlight simulator and retrieves an updated [SimulatorState] in return,
    /// including position, orientation, velocities, and more.
    ///
    /// # Parameters
    ///
    /// - `control`: A [ControlInputs] struct specifying up to 12 RC channels (0.0–1.0 range).
    ///
    /// # Returns
    ///
    /// A [Result] with the updated [SimulatorState] on success, or an error if
    /// something goes wrong (e.g., SOAP fault, network timeout).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use realflight_bridge::{RealFlightBridge, Configuration, ControlInputs};
    /// use std::error::Error;
    ///
    /// fn main() -> Result<(), Box<dyn Error>> {
    ///     let config = Configuration::default();
    ///     let bridge = RealFlightBridge::new(&config)?;
    ///
    ///     // Create sample control inputs
    ///     let mut inputs = ControlInputs::default();
    ///     inputs.channels[0] = 0.5; // Aileron neutral
    ///     inputs.channels[2] = 1.0; // Full throttle
    ///
    ///     // Exchange data with the simulator
    ///     let state = bridge.exchange_data(&inputs)?;
    ///     println!("Current airspeed: {:?}", state.airspeed);
    ///     println!("Altitude above ground: {:?}", state.altitude_agl);
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn exchange_data(&self, control: &ControlInputs) -> Result<SimulatorState, Box<dyn Error>> {
        let body = encode_control_inputs(control);
        let response = self.soap_client.send_action("ExchangeData", &body)?;
        match response.status_code {
            200 => self::decoders::decode_simulator_state(&response.body),
            _ => Err(decode_fault(&response).into()),
        }
    }

    /// Reverts the RealFlight simulator to use its original Spektrum (or built-in) RC input.
    ///
    /// Calling [RealFlightBridge::enable_rc] instructs RealFlight to restore its native RC controller
    /// device (e.g., Spektrum). Once enabled, external RC control via the RealFlight Link
    /// interface is disabled until you explicitly call [RealFlightBridge::disable_rc].
    ///
    /// # Returns
    ///
    /// `Ok(())` if the simulator successfully reverts to using the original RC controller.
    /// An `Err`` is returned if RealFlight cannot locate or restore the original controller device.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use realflight_bridge::{RealFlightBridge, Configuration};
    /// use std::error::Error;
    ///
    /// fn main() -> Result<(), Box<dyn Error>> {
    ///     let config = Configuration::default();
    ///     let bridge = RealFlightBridge::new(&config)?;
    ///
    ///     // Switch back to native Spektrum controller
    ///     bridge.enable_rc()?;
    ///
    ///     // The simulator is now using its default RC input
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn enable_rc(&self) -> Result<(), Box<dyn Error>> {
        self.soap_client
            .send_action("RestoreOriginalControllerDevice", UNUSED)?
            .into()
    }

    /// Switches the RealFlight simulator’s input to the external RealFlight Link controller,
    /// effectively disabling any native Spektrum (or other built-in) RC device.
    ///
    /// Once [RealFlightBridge::disable_rc] is called, RealFlight listens exclusively for commands sent
    /// through this external interface. To revert to the original RC device, call
    /// [RealFlightBridge::enable_rc].
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if RealFlight Link mode is successfully activated, or an `Err` if
    /// the request fails (e.g., simulator is not ready or rejects the command).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use realflight_bridge::{RealFlightBridge, Configuration};
    /// use std::error::Error;
    ///
    /// fn main() -> Result<(), Box<dyn Error>> {
    ///     let config = Configuration::default();
    ///     let bridge = RealFlightBridge::new(&config)?;
    ///
    ///     // Switch to the external RealFlight Link input
    ///     bridge.disable_rc()?;
    ///
    ///     // Now the simulator expects input via RealFlight Link
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn disable_rc(&self) -> Result<(), Box<dyn Error>> {
        self.soap_client
            .send_action("InjectUAVControllerInterface", UNUSED)?
            .into()
    }

    /// Resets the currently loaded aircraft in the RealFlight simulator, analogous
    /// to pressing the spacebar in the simulator’s interface.
    ///
    /// This call repositions the aircraft back to its initial state and orientation,
    /// clearing any damage or off-runway positioning. It’s useful for rapid iteration
    /// when testing control loops or flight maneuvers.
    ///
    /// # Returns
    ///
    /// `Ok(())` upon a successful reset. Returns an error if RealFlight rejects the command
    /// or if a network issue prevents delivery.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use realflight_bridge::{RealFlightBridge, Configuration};
    /// use std::error::Error;
    ///
    /// fn main() -> Result<(), Box<dyn Error>> {
    ///     let config = Configuration::default();
    ///     let bridge = RealFlightBridge::new(&config)?;
    ///
    ///     // Perform a flight test...
    ///     // ...
    ///
    ///     // Reset the aircraft to starting conditions:
    ///     bridge.reset_aircraft()?;
    ///     Ok(())
    /// }
    /// ```
    pub fn reset_aircraft(&self) -> Result<(), Box<dyn Error>> {
        self.soap_client
            .send_action("ResetAircraft", UNUSED)?
            .into()
    }
}

pub(crate) trait SoapClient {
    fn send_action(&self, action: &str, body: &str) -> Result<SoapResponse, Box<dyn Error>>;
    #[cfg(test)]
    fn requests(&self) -> Vec<String> {
        Vec::new()
    }
}

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

const CONTROL_INPUTS_CAPACITY: usize = 291;

fn encode_envelope(action: &str, body: &str) -> String {
    let mut envelope = String::with_capacity(200 + body.len());

    envelope.push_str("<?xml version='1.0' encoding='UTF-8'?>");
    envelope.push_str("<soap:Envelope xmlns:soap='http://schemas.xmlsoap.org/soap/envelope/' xmlns:xsd='http://www.w3.org/2001/XMLSchema' xmlns:xsi='http://www.w3.org/2001/XMLSchema-instance'>");
    envelope.push_str("<soap:Body>");
    envelope.push_str(&format!("<{}>{}</{}>", action, body, action));
    envelope.push_str("</soap:Body>");
    envelope.push_str("</soap:Envelope>");

    envelope
}

fn encode_control_inputs(inputs: &ControlInputs) -> String {
    let mut message = String::with_capacity(CONTROL_INPUTS_CAPACITY);

    message.push_str("<pControlInputs>");
    message.push_str("<m-selectedChannels>4095</m-selectedChannels>");
    //message.push_str("<m-selectedChannels>0</m-selectedChannels>");
    message.push_str("<m-channelValues-0to1>");
    for num in inputs.channels.iter() {
        message.push_str(&format!("<item>{}</item>", num));
    }
    message.push_str("</m-channelValues-0to1>");
    message.push_str("</pControlInputs>");

    message
}

/// Configuration settings for the RealFlight Link bridge.
///
/// The Configuration struct controls how the bridge connects to and communicates with
/// the RealFlight simulator. It provides settings for connection management, timeouts,
/// and performance optimization.
///
/// # Connection Pool
///
/// The bridge maintains a pool of TCP connections to improve performance when making
/// frequent SOAP requests. The pool size and connection behavior can be tuned using
/// the `buffer_size`, `connect_timeout`, and `retry_delay` parameters.
///
/// # Default Configuration
///
/// The default configuration is suitable for most local development:
/// ```rust
/// use realflight_bridge::Configuration;
/// use std::time::Duration;
///
/// let default_config = Configuration {
///     simulator_host: "127.0.0.1:18083".to_string(),
///     connect_timeout: Duration::from_millis(50),
///     pool_size: 1,
/// };
/// ```
///
/// # Examples
///
/// Basic configuration for local development:
/// ```rust
/// use realflight_bridge::Configuration;
/// use std::time::Duration;
///
/// let config = Configuration::default();
/// ```
///
/// Configuration optimized for high-frequency control:
/// ```rust
/// use realflight_bridge::Configuration;
/// use std::time::Duration;
///
/// let config = Configuration {
///     simulator_host: "127.0.0.1:18083".to_string(),
///     connect_timeout: Duration::from_millis(25),  // Faster timeout
///     pool_size: 5,                                // Larger connection pool
/// };
/// ```
///
/// Configuration for a different network interface:
/// ```rust
/// use realflight_bridge::Configuration;
/// use std::time::Duration;
///
/// let config = Configuration {
///     simulator_host: "192.168.1.100:18083".to_string(),
///     connect_timeout: Duration::from_millis(100), // Longer timeout for network
///     pool_size: 2,
/// };
/// ```
#[derive(Clone, Debug)]
pub struct Configuration {
    /// The host where the RealFlight simulator is listening for connections.
    ///
    /// # Format
    /// The value should be in the format "host:port". For local development,
    /// this is typically "127.0.0.1:18083".
    ///
    /// # Important Notes
    /// * The bridge should run on the same machine as RealFlight for best performance
    /// * Remote connections may experience significant latency due to SOAP overhead
    pub simulator_host: String,

    /// Maximum time to wait when establishing a new TCP connection.
    ///
    /// # Performance Impact
    /// * Lower values improve responsiveness when the simulator is unavailable
    /// * Too low values may cause unnecessary connection failures
    /// * Recommended range: 25-100ms for local connections
    ///
    /// # Default
    /// 50 milliseconds
    pub connect_timeout: Duration,

    /// Size of the connection pool.
    ///
    /// The connection pool maintains a set of pre-established TCP connections
    /// to improve performance when making frequent requests to the simulator.
    ///
    /// # Performance Impact
    /// * Larger values can improve throughput for frequent state updates
    /// * Too large values may waste system resources
    /// * Recommended range: 1-5 connections
    ///
    /// # Memory Usage
    /// Each connection in the pool consumes system resources:
    /// * TCP socket
    /// * Memory for connection management
    /// * System file descriptors
    ///
    /// # Default
    /// 1 connection
    pub pool_size: usize,
}

impl Default for Configuration {
    fn default() -> Self {
        Configuration {
            simulator_host: "127.0.0.1:18083".to_string(),
            connect_timeout: Duration::from_millis(50),
            pool_size: 1,
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
/// use realflight_bridge::{RealFlightBridge, Configuration};
/// use std::error::Error;
///
/// fn main() -> Result<(), Box<dyn Error>> {
///     let config = Configuration::default();
///     let bridge = RealFlightBridge::new(&config)?;
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
