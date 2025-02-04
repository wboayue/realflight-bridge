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

use log::error;
use std::error::Error;
use std::io::BufReader;
use std::io::Write;
use std::io::{BufRead, Read};
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use std::net::TcpStream;
use std::time::Duration;
use std::time::Instant;
use uom::si::f64::*;

mod connection_manager;

use connection_manager::ConnectionManager;

const UNUSED: &str = "";
const HEADER_LEN: usize = 120;

/// RealFlightLink client
pub struct RealFlightBridge {
    connection_manager: ConnectionManager,
    statistics: Arc<StatisticsEngine>,
}

impl RealFlightBridge {
    /// Creates a new RealFlightLink client
    /// simulator_url: the url to the RealFlight simulator
    pub fn new(configuration: Configuration) -> Result<RealFlightBridge, Box<dyn Error>> {
        let statistics = Arc::new(StatisticsEngine::new());

        Ok(RealFlightBridge {
            connection_manager: ConnectionManager::new(configuration, statistics.clone())?,
            statistics,
        })
    }

    /// Get statistics for the RealFlightBridge
    pub fn statistics(&self) -> Statistics {
        self.statistics.snapshot()
    }

    /// Reset Real Flight simulator,
    pub fn activate(&self) -> Result<(), Box<dyn Error>> {
        self.reset_aircraft()?;
        self.disable_rc()?;
        Ok(())
    }

    /// Exchange data with the RealFlight simulator
    pub fn exchange_data(&self, control: &ControlInputs) -> Result<SimulatorState, Box<dyn Error>> {
        let body = encode_control_inputs(control);
        let response = self.send_action("ExchangeData", &body)?;
        //        println!("Response: {}", response);
        decode_simulator_state(&response)
    }

    ///  Set Spektrum as the RC input
    fn enable_rc(&self) -> Result<(), Box<dyn Error>> {
        self.send_action("RestoreOriginalControllerDevice", UNUSED)?;
        Ok(())
    }

    /// Disable Spektrum as the RC input, and use FlightAxis instead
    fn disable_rc(&self) -> Result<(), Box<dyn Error>> {
        let _ = self.send_action("InjectUAVControllerInterface", UNUSED)?;
        Ok(())
    }

    /// Reset Real Flight simulator,
    /// like pressing spacebar in the simulator
    pub fn reset_aircraft(&self) -> Result<(), Box<dyn Error>> {
        let _ = self.send_action("ResetAircraft", UNUSED)?;
        Ok(())
    }

    fn send_action(&self, action: &str, body: &str) -> Result<String, Box<dyn Error>> {
        let envelope = encode_envelope(action, body);
        let mut stream = self.connection_manager.get_connection()?;
        self.send_request(&mut stream, action, &envelope);
        self.statistics.increment_request_count();

        match self.read_response(&mut BufReader::new(stream)) {
            Some(response) => {
                // println!("Response: {:?}", response);
                Ok(response.body)
            }
            None => Err("Failed to read response".into()),
        }
    }

    fn send_request(&self, stream: &mut TcpStream, action: &str, envelope: &str) {
        let mut request = String::with_capacity(HEADER_LEN + envelope.len() + action.len());

        request.push_str("POST / HTTP/1.1\r\n");
        request.push_str(&format!("Soapaction: '{}'\r\n", action));
        request.push_str(&format!("Content-Length: {}\r\n", envelope.len()));
        request.push_str("Content-Type: text/xml;charset=utf-8\r\n");
        request.push_str("Connection: Keep-Alive\r\n");
        request.push_str("\r\n");
        request.push_str(envelope);

        stream.write_all(request.as_bytes()).unwrap();
    }

    fn read_response(&self, stream: &mut BufReader<TcpStream>) -> Option<SoapResponse> {
        // let mut buf = String::new();
        // stream.read_to_string(&mut buf).unwrap();
        // println!("Reading response:\n{}", buf);
        // Read the status line
        let mut status_line = String::new();
        stream.read_line(&mut status_line).unwrap();
        eprintln!("Status Line ??: {}", status_line.trim());
        let status_code: u32 = status_line
            .split_whitespace()
            .nth(1)
            .unwrap()
            .parse()
            .unwrap();

        // Read headers
        let mut headers = String::new();
        let mut content_length: Option<usize> = None;
        loop {
            let mut line = String::new();
            stream.read_line(&mut line).unwrap();
            if line == "\r\n" {
                break; // End of headers
            }
            if line.to_lowercase().starts_with("content-length:") {
                if let Some(length) = line.split_whitespace().nth(1) {
                    content_length = length.trim().parse().ok();
                }
            }
            headers.push_str(&line);
        }

        // println!("Headers:\n{}", headers);
        // println!("content length:\n{}", content_length.unwrap());

        // Read the body based on Content-Length
        if let Some(length) = content_length {
            let mut body = vec![0; length];
            stream.read_exact(&mut body).unwrap();
            let body = String::from_utf8_lossy(&body).to_string();
            // println!("Body: {}", r);
            Some(SoapResponse { status_code, body })
        } else {
            None
        }
    }
}

impl Drop for RealFlightBridge {
    fn drop(&mut self) {
        if let Err(e) = self.enable_rc() {
            error!("Error enabling RC: {}", e);
        }
    }
}

#[derive(Debug)]
struct SoapResponse {
    status_code: u32,
    body: String,
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

fn decode_simulator_state(response: &str) -> Result<SimulatorState, Box<dyn Error>> {
    //    println!("Response: {}", response);
    Ok(SimulatorState::default())
}

/// Configuration for the RealFlightBridge
#[derive(Clone, Debug)]
pub struct Configuration {
    pub simulator_url: String,
    pub connect_timeout: Duration,
    pub retry_delay: Duration,
    pub buffer_size: usize,
}

impl Default for Configuration {
    fn default() -> Self {
        Configuration {
            simulator_url: "127.0.0.1:18083".to_string(),
            connect_timeout: Duration::from_millis(50),
            retry_delay: Duration::from_millis(50),
            buffer_size: 1,
        }
    }
}

/// Control inputs for the RealFlight simulator
#[derive(Default, Debug)]
pub struct ControlInputs {
    pub channels: [f32; 12],
}

/// State of the RealFlight simulator
#[derive(Default, Debug)]
pub struct SimulatorState {
    pub previous_inputs: ControlInputs,
    pub airspeed: Velocity,
    pub altitude_asl: Length,
    pub altitude_agl: Length,
    pub groundspeed: Velocity,
    pub pitch_rate: AngularVelocity,
    pub roll_rate: AngularVelocity,
    pub yaw_rate: AngularVelocity,
    pub azimuth: Angle,
    pub inclination: Angle,
    pub roll: Angle,
    pub aircraft_position_x: Length,
    pub aircraft_position_y: Length,
    pub velocity_world_u: Velocity,
    pub velocity_world_v: Velocity,
    pub velocity_world_w: Velocity,
    pub velocity_body_u: Velocity,
    pub velocity_body_v: Velocity,
    pub velocity_body_w: Velocity,
    pub acceleration_world_ax: Acceleration,
    pub acceleration_world_ay: Acceleration,
    pub acceleration_world_az: Acceleration,
    pub acceleration_body_ax: Acceleration,
    pub acceleration_body_ay: Acceleration,
    pub acceleration_body_az: Acceleration,
    pub wind_x: Velocity,
    pub wind_y: Velocity,
    pub wind_z: Velocity,
    pub prop_rpm: Frequency,
    pub heli_main_rotor_rpm: Frequency,
    pub battery_voltage: ElectricPotential,
    pub battery_current_draw: ElectricCurrent,
    pub battery_remaining_capacity: ElectricCharge,
    pub fuel_remaining: Volume,
    pub is_locked: bool,
    pub has_lost_components: bool,
    pub an_engine_is_running: bool,
    pub is_touching_ground: bool,
    pub current_aircraft_status: String,
    pub current_physics_time: Time,
    pub current_physics_speed_multiplier: f64,
    pub orientation_quaternion_x: f64,
    pub orientation_quaternion_y: f64,
    pub orientation_quaternion_z: f64,
    pub orientation_quaternion_w: f64,
    pub flight_axis_controller_is_active: bool,
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

#[cfg(test)]
mod tests;
