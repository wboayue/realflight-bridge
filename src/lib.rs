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

// use decoders::decode_simulator_state;
use soap_client::tcp::TcpSoapClient;
use std::time::Duration;
use std::time::Instant;
use uom::si::f64::*;

#[cfg(test)]
use soap_client::stub::StubSoapClient;

#[cfg(not(any(test, feature = "bench-internals")))]
use decoders::extract_element;

#[cfg(any(test, feature = "bench-internals"))]
pub use decoders::{decode_simulator_state, extract_element, extract_elements};

mod decoders;
mod soap_client;

const UNUSED: &str = "";

/// RealFlightLink client
pub struct RealFlightBridge {
    statistics: Arc<StatisticsEngine>,
    soap_client: Box<dyn SoapClient>,
}

impl RealFlightBridge {
    /// Creates a new RealFlightLink client
    /// simulator_url: the url to the RealFlight simulator
    pub fn new(configuration: Configuration) -> Result<RealFlightBridge, Box<dyn Error>> {
        let statistics = Arc::new(StatisticsEngine::new());

        Ok(RealFlightBridge {
            statistics: statistics.clone(),
            soap_client: Box::new(TcpSoapClient::new(configuration, statistics.clone())?),
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

    /// Exchange data with the RealFlight simulator
    pub fn exchange_data(&self, control: &ControlInputs) -> Result<SimulatorState, Box<dyn Error>> {
        let body = encode_control_inputs(control);
        let response = self.soap_client.send_action("ExchangeData", &body)?;
        match response.status_code {
            200 => self::decoders::decode_simulator_state(&response.body),
            _ => Err(decode_fault(&response).into()),
        }
    }

    ///  Set Spektrum as the RC input
    pub fn enable_rc(&self) -> Result<(), Box<dyn Error>> {
        self.soap_client
            .send_action("RestoreOriginalControllerDevice", UNUSED)?
            .into()
    }

    /// Disable Spektrum as the RC input, and use FlightAxis instead
    pub fn disable_rc(&self) -> Result<(), Box<dyn Error>> {
        self.soap_client
            .send_action("InjectUAVControllerInterface", UNUSED)?
            .into()
    }

    /// Reset Real Flight simulator,
    /// like pressing spacebar in the simulator
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
///     simulator_url: "127.0.0.1:18083".to_string(),
///     connect_timeout: Duration::from_millis(50),
///     retry_delay: Duration::from_millis(50),
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
///     simulator_url: "127.0.0.1:18083".to_string(),
///     connect_timeout: Duration::from_millis(25),  // Faster timeout
///     retry_delay: Duration::from_millis(10),      // Quick retry
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
///     simulator_url: "192.168.1.100:18083".to_string(),
///     connect_timeout: Duration::from_millis(100), // Longer timeout for network
///     retry_delay: Duration::from_millis(100),     // Longer retry for network
///     pool_size: 2,
/// };
/// ```
#[derive(Clone, Debug)]
pub struct Configuration {
    /// The URL where the RealFlight simulator is listening for connections.
    ///
    /// # Format
    /// The URL should be in the format "host:port". For local development,
    /// this is typically "127.0.0.1:18083".
    ///
    /// # Important Notes
    /// * The bridge should run on the same machine as RealFlight for best performance
    /// * Remote connections may experience significant latency due to SOAP overhead
    pub simulator_url: String,

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

    /// Time to wait between connection retry attempts.
    ///
    /// This delay helps prevent overwhelming the system when the simulator
    /// is not responding or during connection pool maintenance.
    ///
    /// # Performance Impact
    /// * Lower values allow faster recovery from connection failures
    /// * Too low values may impact system performance
    /// * Recommended range: 10-100ms
    ///
    /// # Default
    /// 50 milliseconds
    pub retry_delay: Duration,

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
            simulator_url: "127.0.0.1:18083".to_string(),
            connect_timeout: Duration::from_millis(50),
            retry_delay: Duration::from_millis(50),
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
#[derive(Default, Debug)]
pub struct ControlInputs {
    /// Array of 12 channel values, each between 0.0 and 1.0
    pub channels: [f32; 12],
}

/// Represents the complete state of the simulated aircraft in RealFlight.
/// All physical quantities use SI units through the `uom` crate.
#[derive(Default, Debug)]
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
    pub prop_rpm: f64,
    /// Main rotor RPM for helicopters
    pub heli_main_rotor_rpm: f64,
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
    pub current_physics_speed_multiplier: f64,
    /// Quaternion X component (scalar)
    pub orientation_quaternion_x: f64,
    /// Quaternion Y component (scalar)
    pub orientation_quaternion_y: f64,
    /// Quaternion Z component (scalar)
    pub orientation_quaternion_z: f64,
    /// Quaternion W component (scalar)
    pub orientation_quaternion_w: f64,
    /// True if external flight controller is active
    pub flight_axis_controller_is_active: bool,
    /// True if reset button was pressed
    pub reset_button_has_been_pressed: bool,
}

/// Statistics for the RealFlightBridge
#[derive(Debug)]
pub struct Statistics {
    pub runtime: Duration,
    pub error_count: u32,
    pub frame_rate: f64,
    pub request_count: u32,
}

/// Statistics for the RealFlightBridge
pub struct StatisticsEngine {
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
            frame_rate: self.frame_rate(),
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

    fn frame_rate(&self) -> f64 {
        self.request_count() as f64 / self.start_time.elapsed().as_secs_f64()
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
