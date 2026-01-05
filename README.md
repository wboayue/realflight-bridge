# realflight-bridge

*A Rust library to interface external flight controllers with the RealFlight simulator.*

[![Build](https://github.com/wboayue/realflight-bridge/workflows/build/badge.svg)](https://github.com/wboayue/realflight-bridge/actions/workflows/build.yaml)
[![License:MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![crates.io](https://img.shields.io/crates/v/realflight-bridge.svg)](https://crates.io/crates/realflight-bridge)
[![Documentation](https://img.shields.io/badge/Documentation-green.svg)](https://docs.rs/realflight-bridge/latest/realflight_bridge/index.html)
[![Coverage Status](https://coveralls.io/repos/github/wboayue/realflight-bridge/badge.svg?branch=main)](https://coveralls.io/github/wboayue/realflight-bridge?branch=main)

## Overview

[RealFlight](https://www.realflight.com/) is a leading RC flight simulator that provides a realistic, physics-based environment for flying fixed-wing aircraft, helicopters, and drones. Used by both hobbyists and professionals, it simulates aerodynamics, wind conditions, and control responses, making it an excellent tool for flight control algorithm validation.

**RealFlightBridge** is a Rust library that interfaces with [RealFlight Link](https://forums.realflight.com/index.php?threads/flightaxis-link-q-a.32854/), enabling external flight controllers to interact with the simulator. It allows developers to:

* Send control commands to simulated aircraft.
* Receive real-time simulated flight data for state estimation and control.
* Test stabilization and autonomy algorithms in a controlled environment.

## Prerequisites

- [RealFlight simulator](https://www.realflight.com/) (tested with RealFlight Evolution)
- RealFlight Link enabled in simulator settings
  1. Open RealFlight
  2. Go to Settings > Physics -> Quality -> RealFlight Link Enabled
  3. Enable RealFlight Link

The simulator requires a restart after enabling RealFlight Link.

## Install

Use the latest version directly from crates.io:

```bash
cargo add realflight-bridge
```

## Architecture

This library provides two main ways to connect to RealFlight:

1. **Local Bridge**: Connect directly to RealFlight running on the same machine
2. **Remote Bridge**: Connect to RealFlight running on a different machine via a proxy server

## Usage Examples

### Local Connection

RealFlight Link implements a SOAP API that requires a new connection for each request, this introduces significant overhead with non-local connections. Since connecting via the loopback interface has minimal overhead, running the bridge on the same host as the simulator is the recommended approach.

The following example demonstrates how to connect to RealFlight Link, set up the simulation, and send control inputs while receiving simulator state feedback.

```rust
use std::error::Error;

use realflight_bridge::{Configuration, ControlInputs, RealFlightBridge, RealFlightLocalBridge};

pub fn main() -> Result<(), Box<dyn Error>> {
    // Creates bridge with default configuration (connects to 127.0.0.1:18083)
    let bridge = RealFlightLocalBridge::new()?;

    // Reset the simulation to start from a known state
    bridge.reset_aircraft()?;

    // Disable RC input and enable external control
    bridge.disable_rc()?;

    // Initialize control inputs (12 channels available)
    let mut controls: ControlInputs = ControlInputs::default();
    // sim_complete is a placeholder condition; replace with your actual simulation completion logic.
    let mut sim_complete = false;

    loop {
        // Send control inputs and receive simulator state
        let state = bridge.exchange_data(&controls)?;

        // Update control values based on state...
        controls.channels[0] = 0.5; // Example: set first channel to 50%

        if sim_complete {
          bridge.enable_rc()?;
          break;
        }
    }
}
```

### Remote Connection

There are some cases where we may want to run the bridge on a computer that is not running the RealFlight simulator.
For example, you may be developing on a Mac while RealFlight runs only on Windows. To support this scenario, a proxy with an efficient communication protocol was created to forward messages from a remote computer to the simulator via RealFlightBridge. This still requires a low-latency connection. It works well on wired networks or when a Mac communicates with the simulator hosted in a Parallels VM; however, I could not achieve a high enough loop frequency (you want at least 200Hz) over WiFi.

#### Remote Connection (Server)

On the same machine running the RealFlight simulator, install the proxy using the following command.

```bash
cargo install realflight-bridge
```

You can then run it using the following command.

```bash
realflight_bridge_proxy
```

By default, `realflight_bridge_proxy` binds to `0.0.0.0:8080`. This can be changed by passing the `--bind-address` argument to `realflight_bridge_proxy`.

#### Remote Connection (Client)

The following example shows how your application code connects to the simulator using the proxy.

```rust
use std::error::Error;
use realflight_bridge::{RealFlightBridge, RealFlightRemoteBridge, ControlInputs};

fn main() -> Result<(), Box<dyn Error>> {
  let mut client = RealFlightRemoteBridge::new("192.168.12.253:8080")?;

  // Disable RC input and enable external control
  client.disable_rc()?;

  // Initialize control inputs
  let control = ControlInputs::default();

  // Send control inputs and receive update state.
  let state = client.exchange_data(&control)?;

  Ok(())
}
```

## Control Channels

The ControlInputs struct provides 12 channels for aircraft control. Each channel value should be set between 0.0 and 1.0, where:

* 0.0 represents the minimum value
* 0.5 represents the neutral/center position (for control surfaces)
* 1.0 represents the maximum value

## SimulatorState

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

## Sources

The following sources were useful in understanding the RealFlight Link SOAP API:

* RealFlight [developer forums](https://forums.realflight.com/index.php?threads/flightaxis-link-q-a.32854/)
* ArduPilot RealFlight SITL: [SIM_FlightAxis.h](https://github.com/ArduPilot/ardupilot/blob/master/libraries/SITL/SIM_FlightAxis.h), [SIM_FlightAxis.cpp](https://github.com/ArduPilot/ardupilot/blob/master/libraries/SITL/SIM_FlightAxis.cpp)
* Python [Flight Axis implementation](https://github.com/camdeno/F16Capstone/blob/main/FlightAxis/flightaxis.py)

## License

This project is licensed under the MIT License - see the [LICENSE](https://github.com/wboayue/realflight-bridge/blob/pre-release/LICENSE) file for details.