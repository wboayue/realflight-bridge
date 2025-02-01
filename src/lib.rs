/// https://github.com/camdeno/F16Capstone/blob/main/FlightAxis/flightaxis.py
//REALFLIGHT_URL = "http://192.168.55.54:18083"
use std::error::Error;
use std::io::BufReader;
use std::io::Write;
use std::io::{Read, BufRead};
use std::io;
use std::net::{ToSocketAddrs, UdpSocket};
use std::time::Duration;

use log::debug;
use uom::si::f64::*;
use std::net::TcpStream;


pub mod connection_manager;

pub use connection_manager::{ConnectionConfig};

use connection_manager::ConnectionManager;

const UNUSED: &str = "";
const HEADER_LEN: usize = 120;

pub struct RealFlightLink {
    simulator_url: String,
    connection_manager: ConnectionManager,
}

impl RealFlightLink {
    /// Creates a new RealFlightLink client
    /// simulator_url: the url to the RealFlight simulator
    pub fn connect(simulator_url: &str) -> Result<RealFlightLink, Box<dyn Error>> {
        let config = ConnectionConfig {
            simulator_url: simulator_url.to_string(),
            ..Default::default()
        };
        
        Ok(RealFlightLink {
            simulator_url: simulator_url.to_string(),
            connection_manager: ConnectionManager::new(config)?,
        })
    }

    ///  Set Spektrum as the RC input
    pub fn enable_rc(&mut self) -> Result<(), Box<dyn Error>> {
        self.send_action("RestoreOriginalControllerDevice", UNUSED)?;
        Ok(())
    }

    /// Disable Spektrum as the RC input, and use FlightAxis instead
    pub fn disable_rc(&mut self) -> Result<(), Box<dyn Error>> {
        self.send_action("InjectUAVControllerInterface", UNUSED)?;
        Ok(())
    }

    /// Reset Real Flight simulator,
    /// per post here: https://www.knifeedge.com/forums/index.php?threads/realflight-reset-soap-envelope.52333/
    pub fn reset_sim(&mut self) -> Result<(), Box<dyn Error>> {
        let response = self.send_action("ResetAircraft", UNUSED)?;
  //      println!("Response: {}", response);
        Ok(())
    }

    pub fn exchange_data(&mut self, control: &ControlInputs) -> Result<SimulatorState, Box<dyn Error>> {
        let body = encode_control_inputs(control);
        let response = self.send_action("ExchangeData", &body)?;
//        println!("Response: {}", response);
        decode_simulator_state(&response)
    }

    pub fn send_action(&mut self, action: &str, body: &str) -> Result<String, Box<dyn Error>> {
        let envelope = encode_envelope(action, body);
        let mut stream = self.connection_manager.get_connection()?;
        self.send_request(&mut stream, action, &envelope);
        Ok(self.read_response(&mut BufReader::new(stream)))
    }

    pub fn send_request(&mut self, stream: &mut TcpStream, action: &str, envelope: &str) {
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

    pub fn read_response(&mut self, stream: &mut BufReader<TcpStream>) -> String {
        // Read the status line
        let mut status_line = String::new();
        stream.read_line(&mut status_line).unwrap();
        //println!("Status Line: {}", status_line.trim());

        // Read headers
        let mut headers = String::new();
        let mut content_length: Option<usize> = None;
        let mut close_connection = false;
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
            if line.to_lowercase().starts_with("connection:") {
//                println!("Connection: {:?}", line);
                if let Some(length) = line.split_whitespace().nth(1) {
                    let close: Option<String> = length.trim().parse().ok();
                    if close == Some("close".to_string()) {
                        close_connection = true;
                    }
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
            if close_connection {
                self.reset_connection();
            }
            let r = String::from_utf8_lossy(&body).to_string();
            // println!("Body: {}", r);
            r
        } else {
            if close_connection {
                self.reset_connection();
            }
            "".to_string()
        }
    }

    fn reset_connection(&mut self) {
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

fn decode_simulator_state(response: &str) -> Result<SimulatorState, Box<dyn Error>> {
    Ok(SimulatorState::default())
}

#[derive(Default, Debug)]
pub struct ControlInputs {
    pub channels: [f32; 12],
}

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

#[cfg(test)]
mod tests;


/* 
https://github.com/ArduPilot/ardupilot/blob/6bf29eca700120153d7404af1f397c2979715427/libraries/SITL/SIM_FlightAxis.cpp#L234
*/