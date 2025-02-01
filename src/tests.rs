use super::*;

#[test]
pub fn test_encode_control_inputs() {
    let soap_body = "<pControlInputs><m-selectedChannels>4095</m-selectedChannels><m-channelValues-0to1><item>0</item><item>0</item><item>0</item><item>0</item><item>0</item><item>0</item><item>0</item><item>0</item><item>0</item><item>0</item><item>0</item><item>0</item></m-channelValues-0to1></pControlInputs>";
    let control = ControlInputs::default();
    assert_eq!(encode_control_inputs(&control), soap_body);
}

#[test]
pub fn test_encode_envelope() {
    let envelope = "<?xml version='1.0' encoding='UTF-8'?><soap:Envelope xmlns:soap='http://schemas.xmlsoap.org/soap/envelope/' xmlns:xsd='http://www.w3.org/2001/XMLSchema' xmlns:xsi='http://www.w3.org/2001/XMLSchema-instance'><soap:Body><InjectUAVControllerInterface><a>1</a><b>2</b></InjectUAVControllerInterface><soap:Body><soap:Envelope>";
    assert_eq!(
        encode_envelope("InjectUAVControllerInterface", UNUSED),
        envelope
    );
}
