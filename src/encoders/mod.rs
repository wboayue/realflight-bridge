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
mod tests;
