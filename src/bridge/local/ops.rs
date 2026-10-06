//! RealFlight Link SOAP operations shared by sync and async local bridges.
//! Runtime-agnostic (no I/O): maps operations to requests and responses to results.

use crate::decoders::decode_simulator_state;
use crate::encoders::encode_control_inputs;
use crate::soap_client::SoapResponse;
use crate::{BridgeError, ControlInputs, SimulatorState};

/// A RealFlight Link SOAP operation.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Op<'a> {
    Exchange(&'a ControlInputs),
    EnableRc,
    DisableRc,
    Reset,
}

impl Op<'_> {
    /// SOAP action name.
    pub(crate) fn action(&self) -> &'static str {
        match self {
            Op::Exchange(_) => "ExchangeData",
            Op::EnableRc => "RestoreOriginalControllerDevice",
            Op::DisableRc => "InjectUAVControllerInterface",
            Op::Reset => "ResetAircraft",
        }
    }

    /// SOAP request body (contents of the action element).
    pub(crate) fn body(&self) -> String {
        match self {
            Op::Exchange(control) => encode_control_inputs(control),
            Op::EnableRc | Op::DisableRc | Op::Reset => String::new(),
        }
    }
}

/// Decodes an `ExchangeData` response into simulator state.
pub(crate) fn decode_exchange(response: SoapResponse) -> Result<SimulatorState, BridgeError> {
    decode_simulator_state(&ok_body(response)?)
}

/// Decodes a response to an operation without a payload.
pub(crate) fn decode_unit(response: SoapResponse) -> Result<(), BridgeError> {
    ok_body(response).map(|_| ())
}

/// Returns the body of a successful response, or the SOAP fault.
fn ok_body(response: SoapResponse) -> Result<String, BridgeError> {
    match response.status_code {
        200 => Ok(response.body),
        _ => Err(BridgeError::SoapFault(response.fault_message())),
    }
}

#[cfg(test)]
mod tests;
