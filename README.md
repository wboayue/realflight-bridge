[![Build](https://github.com/wboayue/realflight-link/workflows/build/badge.svg)](https://github.com/wboayue/realflight-link/actions/workflows/build.yaml)
[![License:MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
<!-- [![crates.io](https://img.shields.io/crates/v/ibapi.svg)](https://crates.io/crates/ibapi)
[![Documentation](https://img.shields.io/badge/Documentation-green.svg)](https://docs.rs/ibapi/latest/ibapi/)
[![Coverage Status](https://coveralls.io/repos/github/wboayue/rust-ibapi/badge.svg?branch=main)](https://coveralls.io/github/wboayue/rust-ibapi?branch=main) -->


# Overview

[RealFlight](https://www.realflight.com/) is a leading RC flight simulator that provides a realistic, physics-based environment for flying fixed-wing aircraft, helicopters, and drones. Used by both hobbyists and professionals, it simulates aerodynamics, wind conditions, and control responses, making it an excellent tool for flight control algorithm validation.

This Rust library interfaces with [RealFlight Link](https://forums.realflight.com/index.php?threads/flightaxis-link-q-a.32854/), enabling external flight controllers to interact with the simulator. It allows developers to:

* Send control commands to simulated aircraft.
* Receive real-time simulated flight data for state estimation and control.
* Test stabilization and autonomy algorithms in a controlled environment.

Custom aircraft models can also be created to closely match real-world designs, providing a more precise testing and development platform.

# Prerequisites

- RealFlight simulator (tested with RealFlight Evolution)
- RealFlight Link enabled in simulator settings
  1. Open RealFlight
  2. Go to Settings > Link
  3. Enable external control

# Install

To add `realflight_bridge` to your Rust project, include the following in your `Cargo.toml`:

```toml
[dependencies]
realflight_bridge = "1.0.0"
```

# Example Usage

The following example demonstrates how to connect to RealFlight Link, set up the simulation, and send control inputs in a loop while receiving simulator state feedback.

```rust
use std::error::Error;

use realflight_bridge::{Configuration, ControlInputs, RealFlightBridge};


pub fn main() -> Result<(), Box<dyn Error>> {
    // Creates bridge with default configuration.
    // The default configuration connects to RealFlight Link at 127.0.0.1:18083
    let bridge = RealFlightBridge::new(Configuration::default());

    // Activate the bridge by resetting the simulation and enabling external control input.
    bridge.activate()?;

    // Initialize control inputs.
    let mut controls: ControlInputs = ControlInputs::default();

    loop {
        // Send control inputs and receive simulator state
        let state = bridge.exchange_data(controls)?;

        // Compute new control inputs based on received state.
        controls = compute_new_control(state)?;
    }
}
```

See the documentation for a detailed description of Configuration, ControlInputs and SimulatorState.

# Running the Bridge

The bridge must run on the same computer as the RealFlight simulator. The RealFlight Link SOAP API requires a new connection for each request, which introduces significant overhead. As a result, running the bridge on a remote host will severely limit communication throughput.

If you need to communicate from a remote host, it is recommended to create your own efficient communication protocol that connects your external system to the bridge running on the same host as the RealFlight simulator.

# Sources

The following sources were useful in understanding the RealFlight Link SOAP API:

* RealFlight developer forums
* ArduPilot RealFlight SITL
* Flight axis [python script](https://github.com/camdeno/F16Capstone/blob/main/FlightAxis/flightaxis.py) by Michal Podhradsky
