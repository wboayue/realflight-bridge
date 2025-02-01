/// https://github.com/camdeno/F16Capstone/blob/main/FlightAxis/flightaxis.py
//REALFLIGHT_URL = "http://192.168.55.54:18083"
use std::error::Error;
use std::time::Duration;

use uom::si::f64::*;
use ureq::Agent;

//const UNUSED: &str = "<unused>0</unused>";
const UNUSED: &str = "";

pub struct RealFlightLink {
    simulator_url: String,
    client: ureq::Agent,
}

impl RealFlightLink {
    /// Creates a new RealFlightLink client
    /// simulator_url: the url to the RealFlight simulator
    pub fn new(simulator_url: &str) -> RealFlightLink {
        let mut config = Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(5)))
        .build();

        let agent: Agent = config.into();

        RealFlightLink {
            simulator_url: simulator_url.to_string(),
            client: agent,
        }
    }

    ///  Set Spektrum as the RC input
    pub fn enable_rc(&self) -> Result<(), Box<dyn Error>> {
        self.send_action("RestoreOriginalControllerDevice", UNUSED)?;
        Ok(())
    }

    /// Disable Spektrum as the RC input, and use FlightAxis instead
    pub fn disable_rc(&self) -> Result<(), Box<dyn Error>> {
        self.send_action("InjectUAVControllerInterface", UNUSED)?;
        Ok(())
    }

    /// Reset Real Flight simulator,
    /// per post here: https://www.knifeedge.com/forums/index.php?threads/realflight-reset-soap-envelope.52333/
    pub fn reset_sim(&self) -> Result<(), Box<dyn Error>> {
        self.send_action("ResetAircraft", UNUSED)?;
        Ok(())
    }

    pub fn exchange_data(&self, control: &ControlInputs) -> Result<SimulatorState, Box<dyn Error>> {
        let body = encode_control_inputs(control);
        let response = self.send_action("ExchangeData", &body)?;
        decode_simulator_state(&response)
    }

    pub fn send_action(&self, action: &str, body: &str) -> Result<String, Box<dyn Error>> {
        let envelope = encode_envelope(action, body);
       println!("envelope: {}", envelope);
        let response: String = self
            .client
            .post(&self.simulator_url)
            .header("content-type", "text/xml;charset='UTF-8'")
            .header("soapaction", action)
            .send(envelope)?
            .body_mut()
            .read_to_string()?;

//        println!("response: {}", response);

        Ok(response)
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
    pub current_aircraft_status: u8,
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
    asprintf(&req, R"(POST / HTTP/1.1
soapaction: '%s'
content-length: %u
content-type: text/xml;charset='UTF-8'
Connection: Keep-Alive

%s)",
             action,
             (unsigned)strlen(req1), req1);
    sock->send(req, strlen(req));


https://github.com/ArduPilot/ardupilot/blob/6bf29eca700120153d7404af1f397c2979715427/libraries/SITL/SIM_FlightAxis.cpp#L234
*/