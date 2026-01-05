use std::{error::Error, sync::Arc, time::Duration};

use crate::{
    BridgeError, ControlInputs, SimulatorState, SoapResponse, Statistics, StatisticsEngine,
    TcpSoapClient,
};

#[cfg(test)]
use crate::StubSoapClient;

use super::RealFlightBridge;

const EMPTY_BODY: &str = "";

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
/// use realflight_bridge::{RealFlightBridge, RealFlightLocalBridge, Configuration, ControlInputs};
/// use std::error::Error;
///
/// fn main() -> Result<(), Box<dyn Error>> {
///     // Build a RealFlightBridge client
///     let bridge = RealFlightLocalBridge::new()?;
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
pub struct RealFlightLocalBridge {
    statistics: Arc<StatisticsEngine>,
    soap_client: Box<dyn SoapClient>,
}

impl RealFlightBridge for RealFlightLocalBridge {
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
    /// use realflight_bridge::{RealFlightBridge, RealFlightLocalBridge, Configuration, ControlInputs};
    /// use std::error::Error;
    ///
    /// fn main() -> Result<(), Box<dyn Error>> {
    ///     let bridge = RealFlightLocalBridge::new()?;
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
    fn exchange_data(&self, control: &ControlInputs) -> Result<SimulatorState, BridgeError> {
        let body = encode_control_inputs(control);
        let response = self
            .soap_client
            .send_action("ExchangeData", &body)
            .map_err(|e| BridgeError::SoapFault(e.to_string()))?;
        match response.status_code {
            200 => crate::decoders::decode_simulator_state(&response.body),
            _ => Err(BridgeError::SoapFault(crate::decode_fault(&response))),
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
    /// use realflight_bridge::{RealFlightBridge, RealFlightLocalBridge, Configuration};
    /// use std::error::Error;
    ///
    /// fn main() -> Result<(), Box<dyn Error>> {
    ///     let bridge = RealFlightLocalBridge::new()?;
    ///
    ///     // Switch back to native Spektrum controller
    ///     bridge.enable_rc()?;
    ///
    ///     // The simulator is now using its default RC input
    ///
    ///     Ok(())
    /// }
    /// ```
    fn enable_rc(&self) -> Result<(), BridgeError> {
        self.soap_client
            .send_action("RestoreOriginalControllerDevice", EMPTY_BODY)
            .map_err(|e| BridgeError::SoapFault(e.to_string()))?
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
    /// use realflight_bridge::{RealFlightBridge, RealFlightLocalBridge, Configuration};
    /// use std::error::Error;
    ///
    /// fn main() -> Result<(), Box<dyn Error>> {
    ///     let bridge = RealFlightLocalBridge::new()?;
    ///
    ///     // Switch to the external RealFlight Link input
    ///     bridge.disable_rc()?;
    ///
    ///     // Now the simulator expects input via RealFlight Link
    ///
    ///     Ok(())
    /// }
    /// ```
    fn disable_rc(&self) -> Result<(), BridgeError> {
        self.soap_client
            .send_action("InjectUAVControllerInterface", EMPTY_BODY)
            .map_err(|e| BridgeError::SoapFault(e.to_string()))?
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
    /// use realflight_bridge::{RealFlightBridge, RealFlightLocalBridge, Configuration};
    /// use std::error::Error;
    ///
    /// fn main() -> Result<(), Box<dyn Error>> {
    ///     let bridge = RealFlightLocalBridge::new()?;
    ///
    ///     // Perform a flight test...
    ///     // ...
    ///
    ///     // Reset the aircraft to starting conditions:
    ///     bridge.reset_aircraft()?;
    ///     Ok(())
    /// }
    /// ```
    fn reset_aircraft(&self) -> Result<(), BridgeError> {
        self.soap_client
            .send_action("ResetAircraft", EMPTY_BODY)
            .map_err(|e| BridgeError::SoapFault(e.to_string()))?
            .into()
    }
}

impl RealFlightLocalBridge {
    /// Creates a new [RealFlightBridge] instance configured to communicate
    /// with a RealFlight simulator running on local machine.
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
    /// use realflight_bridge::{RealFlightLocalBridge, Configuration};
    /// use std::error::Error;
    ///
    /// fn main() -> Result<(), Box<dyn Error>> {
    ///     // Build a bridge to the RealFlight simulator.
    ///     // Connects to simulator at 127.0.0.1:18083
    ///     let bridge = RealFlightLocalBridge::new()?;
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
    /// - If the TCP connection pool cannot be established (e.g., RealFlight is not running).
    pub fn new() -> Result<RealFlightLocalBridge, Box<dyn Error>> {
        let statistics = Arc::new(StatisticsEngine::new());
        let soap_client = TcpSoapClient::new(Configuration::default(), statistics.clone())?;
        soap_client.ensure_pool_initialized()?;

        Ok(RealFlightLocalBridge {
            statistics: statistics.clone(),
            soap_client: Box::new(soap_client),
        })
    }

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
    /// use realflight_bridge::{RealFlightLocalBridge, Configuration};
    /// use std::error::Error;
    ///
    /// fn main() -> Result<(), Box<dyn Error>> {
    ///     // Use default localhost-based config.
    ///     let config = Configuration::default();
    ///
    ///     // Build a bridge to the RealFlight simulator.
    ///     let bridge = RealFlightLocalBridge::with_configuration(&config)?;
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
    pub fn with_configuration(
        configuration: &Configuration,
    ) -> Result<RealFlightLocalBridge, Box<dyn Error>> {
        let statistics = Arc::new(StatisticsEngine::new());
        let soap_client = TcpSoapClient::new(configuration.clone(), statistics.clone())?;
        soap_client.ensure_pool_initialized()?;

        Ok(RealFlightLocalBridge {
            statistics: statistics.clone(),
            soap_client: Box::new(soap_client),
        })
    }

    /// Creates a new RealFlightLink client
    /// simulator_url: the url to the RealFlight simulator
    #[cfg(test)]
    pub(crate) fn stub(
        mut soap_client: StubSoapClient,
    ) -> Result<RealFlightLocalBridge, Box<dyn Error>> {
        let statistics = Arc::new(StatisticsEngine::new());

        soap_client.statistics = Some(statistics.clone());

        Ok(RealFlightLocalBridge {
            statistics,
            soap_client: Box::new(soap_client),
        })
    }

    #[cfg(test)]
    pub fn requests(&self) -> Vec<String> {
        self.soap_client.requests().clone()
    }

    /// Get statistics for the RealFlightBridge
    pub fn statistics(&self) -> Statistics {
        self.statistics.snapshot()
    }
}

pub(crate) trait SoapClient {
    fn send_action(&self, action: &str, body: &str) -> Result<SoapResponse, Box<dyn Error>>;
    #[cfg(test)]
    fn requests(&self) -> Vec<String> {
        Vec::new()
    }
}

const CONTROL_INPUTS_CAPACITY: usize = 291;

pub(crate) fn encode_envelope(action: &str, body: &str) -> String {
    let mut envelope = String::with_capacity(200 + body.len());

    envelope.push_str("<?xml version='1.0' encoding='UTF-8'?>");
    envelope.push_str("<soap:Envelope xmlns:soap='http://schemas.xmlsoap.org/soap/envelope/' xmlns:xsd='http://www.w3.org/2001/XMLSchema' xmlns:xsi='http://www.w3.org/2001/XMLSchema-instance'>");
    envelope.push_str("<soap:Body>");
    envelope.push_str(&format!("<{}>{}</{}>", action, body, action));
    envelope.push_str("</soap:Body>");
    envelope.push_str("</soap:Envelope>");

    envelope
}

pub(crate) fn encode_control_inputs(inputs: &ControlInputs) -> String {
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
    /// 5 milliseconds
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
            simulator_host: crate::DEFAULT_SIMULATOR_HOST.to_string(),
            connect_timeout: Duration::from_millis(5),
            pool_size: 1,
        }
    }
}
