/// https://github.com/camdeno/F16Capstone/blob/main/FlightAxis/flightaxis.py
//REALFLIGHT_URL = "http://192.168.55.54:18083"

use std::error::Error;

use uom::si::f64::*;

const EMPTY_BODY: &str = "<a>1</a><b>2</b>";

pub struct RealFlightLink {
}

#[derive(Default)]
pub struct ControlInputs {
    pub channels: [f32; 12],
}

#[derive(Default)]
pub struct SimulatorState {
    pub previous_inputs: ControlInputs,
    pub airspeed: Velocity,
    pub altitude_asl: Length,
    pub altitude_agl: Length,
    pub groundspeed: Velocity,
    pub pitch_rate: AngularVelocity,
    pub roll_rate: AngularVelocity,
    // double m_yawRate_DEGpSEC;
    pub azimuth: Angle,
    pub inclination: Angle,
    // double m_roll_DEG;
    // double m_aircraftPositionX_MTR;
    // double m_aircraftPositionY_MTR;
    // double m_velocityWorldU_MPS;
    // double m_velocityWorldV_MPS;
    // double m_velocityWorldW_MPS;
    // double m_velocityBodyU_MPS;
    // double m_velocityBodyV_MPS;
    // double m_velocityBodyW_MPS;
    // double m_accelerationWorldAX_MPS2;
    // double m_accelerationWorldAY_MPS2;
    // double m_accelerationWorldAZ_MPS2;
    // double m_accelerationBodyAX_MPS2;
    // double m_accelerationBodyAY_MPS2;
    // double m_accelerationBodyAZ_MPS2;
    // double m_windX_MPS;
    // double m_windY_MPS;
    // double m_windZ_MPS;
    // double m_propRPM;
    // double m_heliMainRotorRPM;
    // double m_batteryVoltage_VOLTS;
    // double m_batteryCurrentDraw_AMPS;
    // double m_batteryRemainingCapacity_MAH;
    // double m_fuelRemaining_OZ;
    // double m_isLocked;
    // double m_hasLostComponents;
    // double m_anEngineIsRunning;
    // double m_isTouchingGround;
    // double m_currentAircraftStatus;
    // double m_currentPhysicsTime_SEC;
    // double m_currentPhysicsSpeedMultiplier;
    // double m_orientationQuaternion_X;
    // double m_orientationQuaternion_Y;
    // double m_orientationQuaternion_Z;
    // double m_orientationQuaternion_W;
    // double m_flightAxisControllerIsActive;
    // double m_resetButtonHasBeenPressed;
}

impl RealFlightLink {
    pub fn new() -> RealFlightLink {
        RealFlightLink {}
    }

    ///  Set Spektrum as the RC input
    pub fn enable_rc(&self) -> Result<(), Box<dyn Error>> {
        self.send_action("RestoreOriginalControllerDevice", EMPTY_BODY)?;
        Ok(())
    }

    /// Disable Spektrum as the RC input, and use FlightAxis instead
    pub fn disable_rc(&self) -> Result<(), Box<dyn Error>> {
        self.send_action("InjectUAVControllerInterface", EMPTY_BODY)?;
        Ok(())
    }

    /// Reset Real Flight simulator,
    /// per post here: https://www.knifeedge.com/forums/index.php?threads/realflight-reset-soap-envelope.52333/
    pub fn reset_sim(&self) -> Result<(), Box<dyn Error>> {
        self.send_action("ResetAircraft", EMPTY_BODY)?;
        Ok(())
    }

    pub fn exchange_data(&self, control: ControlInputs) -> Result<SimulatorState, Box<dyn Error>> {
        // ExchangeData
        /*          <pControlInputs>\
        <m-selectedChannels>4095</m-selectedChannels>\
        <m-channelValues-0to1>\
        <item>{self.rcin[0]}</item>\
        <item>{self.rcin[1]}</item>\
        <item>{self.rcin[2]}</item>\
        <item>{self.rcin[3]}</item>\
        <item>{self.rcin[4]}</item>\
        <item>{self.rcin[5]}</item>\
        <item>{self.rcin[6]}</item>\
        <item>{self.rcin[7]}</item>\
        <item>{self.rcin[8]}</item>\
        <item>{self.rcin[9]}</item>\
        <item>{self.rcin[10]}</item>\
        <item>{self.rcin[11]}</item>\
        </m-channelValues-0to1>\
        </pControlInputs>\
        */
        Ok(SimulatorState::default())
    }

    pub fn send_action(&self, action: &str, body: &str) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

}

/*
headers = {'content-type': "text/xml;charset='UTF-8'",
                'soapaction': 'InjectUAVControllerInterface'}

        body = "<?xml version='1.0' encoding='UTF-8'?>\
        <soap:Envelope xmlns:soap='http://schemas.xmlsoap.org/soap/envelope/' xmlns:xsd='http://www.w3.org/2001/XMLSchema' xmlns:xsi='http://www.w3.org/2001/XMLSchema-instance'>\
        <soap:Body>\
        <InjectUAVControllerInterface><a>1</a><b>2</b></InjectUAVControllerInterface>\
        </soap:Body>\
        </soap:Envelope>"
*/
#[cfg(test)]
mod tests;
