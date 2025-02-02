# Overview

[RealFlight](https://www.realflight.com/) is a leading RC flight simulator that provides a realistic, physics-based environment for flying fixed-wing aircraft, helicopters, and drones. Used by both hobbyists and professionals, it simulates aerodynamics, wind conditions, and control responses, making it an excellent tool for flight control algorithm validation.

This Rust library interfaces with [RealFlight Link](https://forums.realflight.com/index.php?threads/flightaxis-link-q-a.32854/), enabling external flight controllers to interact with the simulator. It allows developers to:

* Send control commands to simulated aircraft.
* Receive real-time simulated flight data for state estimation and control.
* Test stabilization and autonomy algorithms in a controlled environment.

Custom aircraft models can also be created to closely match real-world designs, providing a more precise testing and development platform.

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

```bash
cargo run --example benchmark -- --simulator_url=192.168.4.117:18083
```

Sources
* forums
* ardupilot src
* /// https://github.com/camdeno/F16Capstone/blob/main/FlightAxis/flightaxis.py

//REALFLIGHT_URL = "http://192.168.55.54:18083"
