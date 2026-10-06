use super::*;

const RETURN_DATA_200: &str = include_str!("../../../../testdata/responses/return-data-200.xml");

fn response(status_code: u32, body: &str) -> SoapResponse {
    SoapResponse {
        status_code,
        body: body.to_string(),
    }
}

mod op {
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

mod decode {
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
