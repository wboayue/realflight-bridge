[![Build](https://github.com/wboayue/realflight-bridge/workflows/build/badge.svg)](https://github.com/wboayue/realflight-bridge/actions/workflows/build.yaml)
[![License:MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![crates.io](https://img.shields.io/crates/v/realflight-bridge.svg)](https://crates.io/crates/realflight-bridge)
[![Documentation](https://img.shields.io/badge/Documentation-green.svg)](https://docs.rs/realflight-bridge/latest/realflight-bridge/)
[![Coverage Status](https://coveralls.io/repos/github/wboayue/realflight-bridge/badge.svg?branch=main)](https://coveralls.io/github/wboayue/realflight-bridge?branch=main)

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
  2. Go to Settings > Physics -> Quality -> RealFlight Link Enabled
  3. Enable RealFlight Link

# Install

To add `realflight_bridge` to your Rust project, include the following in your `Cargo.toml`:

```toml
[dependencies]
realflight_bridge = "0.1.0"
scopeguard = "1.2"       # For safe cleanup in examples
```

# Example Usage

The following example demonstrates how to connect to RealFlight Link, set up the simulation, and send control inputs while receiving simulator state feedback.

```rust
use std::error::Error;

use realflight_bridge::{Configuration, ControlInputs, RealFlightBridge};
use scopeguard;

pub fn main() -> Result<(), Box<dyn Error>> {
    // Creates bridge with default configuration (connects to 127.0.0.1:18083)
    let bridge = RealFlightBridge::new(Configuration::default())?;

    // Reset the simulation to start from a known state
    bridge.reset_aircraft()?;

    // Disable RC input and enable external control
    bridge.disable_rc()?;

    // Ensure RC control is restored even if we panic
    let _cleanup = scopeguard::guard((), |_| {
        if let Err(e) = bridge.enable_rc() {
            eprintln!("Error restoring RC control: {}", e);
        }
    });

    // Initialize control inputs (12 channels available)
    let mut controls: ControlInputs = ControlInputs::default();

    loop {
        // Send control inputs and receive simulator state
        let state = bridge.exchange_data(&controls)?;

        // Update control values based on state...
        controls.channels[0] = 0.5; // Example: set first channel to 50%
    }
}
```

# Control Channels

The ControlInputs struct provides 12 channels for aircraft control. Each channel value should be set between 0.0 and 1.0, where:

* 0.0 represents the minimum value (0%)
* 1.0 represents the maximum value (100%)

# SimulatorState

The SimulatorState struct provides comprehensive flight data including:

* Position and Orientation
  - Aircraft position (X, Y coordinates)
  - Orientation quaternion (X, Y, Z, W)
  - Heading, pitch, and roll angles

* Velocities and Accelerations
  - Airspeed and groundspeed
  - Body and world frame velocities
  - Linear and angular accelerations

* Environment
  - Altitude (ASL and AGL)
  - Wind conditions (X, Y, Z components)

* System Status
  - Battery voltage and current
  - Fuel remaining
  - Engine state
  - Aircraft status messages

All physical quantities use SI units through the `uom` crate.

# Architecture Notes

The bridge must run on the same computer as the RealFlight simulator. The RealFlight Link SOAP API requires a new connection for each request, which introduces significant overhead. As a result, running the bridge on a remote host will severely limit communication throughput.

For remote operation, it is recommended to create your own efficient communication protocol between the remote host and the bridge.

# Sources

The following sources were useful in understanding the RealFlight Link SOAP API:

* RealFlight [developer forums](https://forums.realflight.com/index.php?threads/flightaxis-link-q-a.32854/)
* ArduPilot RealFlight SITL: [SIM_FlightAxis.h](https://github.com/ArduPilot/ardupilot/blob/master/libraries/SITL/SIM_FlightAxis.h), [SIM_FlightAxis.cpp](https://github.com/ArduPilot/ardupilot/blob/master/libraries/SITL/SIM_FlightAxis.cpp)
* Flight Axis [python implementation](https://github.com/camdeno/F16Capstone/blob/main/FlightAxis/flightaxis.py)

# License

This project is licensed under the MIT License - see the [LICENSE](https://github.com/wboayue/realflight-bridge/blob/pre-release/LICENSE) file for details.