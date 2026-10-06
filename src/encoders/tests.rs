use super::*;

#[test]
fn encode_default_inputs() {
    let inputs = ControlInputs::default();
    let encoded = encode_control_inputs(&inputs);

    assert!(encoded.contains("<pControlInputs>"));
    assert!(encoded.contains("<m-selectedChannels>4095</m-selectedChannels>"));
    assert!(encoded.contains("<m-channelValues-0to1>"));
    assert!(encoded.contains("</pControlInputs>"));
}

#[test]
fn encode_has_correct_structure() {
    let inputs = ControlInputs::default();
    let encoded = encode_control_inputs(&inputs);

    assert!(encoded.starts_with("<pControlInputs>"));
    assert!(encoded.ends_with("</pControlInputs>"));
}

#[test]
fn encode_all_twelve_channels() {
    let inputs = ControlInputs::default();
    let encoded = encode_control_inputs(&inputs);

    let item_count = encoded.matches("<item>").count();
    assert_eq!(item_count, 12);
}

#[test]
fn encode_sequential_inputs() {
    let mut inputs = ControlInputs::default();
    for i in 0..12 {
        inputs.channels[i] = i as f32 / 11.0;
    }
    let encoded = encode_control_inputs(&inputs);

    assert!(encoded.contains("<item>0</item>"));
    assert!(encoded.contains("<item>1</item>"));
}

#[test]
fn encode_boundary_values() {
    let mut inputs = ControlInputs::default();
    inputs.channels[0] = 0.0;
    inputs.channels[11] = 1.0;

    let encoded = encode_control_inputs(&inputs);

    assert!(encoded.contains("<item>0</item>"));
    assert!(encoded.contains("<item>1</item>"));
}
