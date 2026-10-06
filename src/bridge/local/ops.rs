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
mod tests {
    use super::*;

    const RETURN_DATA_200: &str = include_str!("../../../testdata/responses/return-data-200.xml");

    fn response(status_code: u32, body: &str) -> SoapResponse {
        SoapResponse {
            status_code,
            body: body.to_string(),
        }
    }

    mod op_tests {
        use super::*;

        #[test]
        fn maps_actions() {
            let control = ControlInputs::default();
            assert_eq!(Op::Exchange(&control).action(), "ExchangeData");
            assert_eq!(Op::EnableRc.action(), "RestoreOriginalControllerDevice");
            assert_eq!(Op::DisableRc.action(), "InjectUAVControllerInterface");
            assert_eq!(Op::Reset.action(), "ResetAircraft");
        }

        #[test]
        fn exchange_body_encodes_control_inputs() {
            let mut control = ControlInputs::default();
            control.channels[0] = 0.5;
            assert_eq!(
                Op::Exchange(&control).body(),
                encode_control_inputs(&control)
            );
        }

        #[test]
        fn unit_ops_have_empty_body() {
            for op in [Op::EnableRc, Op::DisableRc, Op::Reset] {
                assert!(op.body().is_empty(), "{:?} body not empty", op);
            }
        }
    }

    mod decode_tests {
        use super::*;

        #[test]
        fn decode_unit_ok_on_200() {
            assert!(decode_unit(response(200, "")).is_ok());
        }

        #[test]
        fn decode_unit_fault_on_500() {
            match decode_unit(response(500, "<detail>Server error</detail>")) {
                Err(BridgeError::SoapFault(msg)) => assert_eq!(msg, "Server error"),
                other => panic!("expected SoapFault, got {:?}", other),
            }
        }

        #[test]
        fn decode_unit_fault_without_detail() {
            match decode_unit(response(500, "<faultcode>Client</faultcode>")) {
                Err(BridgeError::SoapFault(msg)) => {
                    assert_eq!(msg, "Failed to extract error message")
                }
                other => panic!("expected SoapFault, got {:?}", other),
            }
        }

        #[test]
        fn decode_exchange_returns_state_on_200() {
            let state = decode_exchange(response(200, RETURN_DATA_200)).unwrap();
            assert_eq!(state, decode_simulator_state(RETURN_DATA_200).unwrap());
        }

        #[test]
        fn decode_exchange_fault_on_500() {
            match decode_exchange(response(500, "<detail>Bad inputs</detail>")) {
                Err(BridgeError::SoapFault(msg)) => assert_eq!(msg, "Bad inputs"),
                other => panic!("expected SoapFault, got {:?}", other),
            }
        }
    }
}
