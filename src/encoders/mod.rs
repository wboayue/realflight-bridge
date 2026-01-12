//! Encoding functions for RealFlight simulator protocol.

use std::fmt::Write;

use crate::ControlInputs;

const CONTROL_INPUTS_CAPACITY: usize = 291;

/// Encodes control inputs into XML format for the RealFlight simulator.
#[cfg(any(test, feature = "bench-internals"))]
pub fn encode_control_inputs(inputs: &ControlInputs) -> String {
    encode_control_inputs_inner(inputs)
}

/// Encodes control inputs into XML format for the RealFlight simulator.
#[cfg(not(any(test, feature = "bench-internals")))]
pub(crate) fn encode_control_inputs(inputs: &ControlInputs) -> String {
    encode_control_inputs_inner(inputs)
}

fn encode_control_inputs_inner(inputs: &ControlInputs) -> String {
    let mut message = String::with_capacity(CONTROL_INPUTS_CAPACITY);

    message.push_str("<pControlInputs>");
    message.push_str("<m-selectedChannels>4095</m-selectedChannels>");
    message.push_str("<m-channelValues-0to1>");
    for num in inputs.channels.iter() {
        let _ = write!(message, "<item>{}</item>", num);
    }
    message.push_str("</m-channelValues-0to1>");
    message.push_str("</pControlInputs>");

    message
}

#[cfg(test)]
mod tests {
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
}
