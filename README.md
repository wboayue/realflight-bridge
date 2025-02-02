# Overview

[RealFlight](https://www.realflight.com/) is a leading RC flight simulator that provides a realistic, physics-based environment for flying fixed-wing aircraft, helicopters, and drones. Used by both hobbyists and professionals, it simulates aerodynamics, wind conditions, and control responses, making it an excellent tool for flight control algorithm validation.

This Rust library interfaces with [RealFlight Link](https://forums.realflight.com/index.php?threads/flightaxis-link-q-a.32854/), enabling external flight controllers to interact with the simulator. It allows developers to:

* Send control commands to simulated aircraft.
* Receive real-time simulated flight data for state estimation and control.
* Test stabilization and autonomy algorithms in a controlled environment.

Custom aircraft models can also be created to closely match real-world designs, providing a more precise testing and development platform.

# Install

Add the following to your Cargo.toml

```toml
[dependencies]
realflight-link = "1.0.0"
```

# Examples

RealFlight Link provides a SOAP API with four methods

* RestoreOriginalControllerDevice
* InjectUAVControllerInterface
* ResetAircraft
* ExchangeData

```bash
cargo run --example benchmark -- --simulator_url=192.168.4.117:18083
```

Sources
* forums
* ardupilot src
* /// https://github.com/camdeno/F16Capstone/blob/main/FlightAxis/flightaxis.py

//REALFLIGHT_URL = "http://192.168.55.54:18083"
