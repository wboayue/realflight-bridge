# realflight-bridge

*A Rust library to interface external flight controllers with the RealFlight simulator.*

[![Build](https://github.com/wboayue/realflight-bridge/actions/workflows/build.yaml/badge.svg)](https://github.com/wboayue/realflight-bridge/actions/workflows/build.yaml)
[![License:MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![crates.io](https://img.shields.io/crates/v/realflight-bridge.svg)](https://crates.io/crates/realflight-bridge)
[![Documentation](https://img.shields.io/docsrs/realflight-bridge)](https://docs.rs/realflight-bridge/latest/realflight_bridge/index.html)
[![Coverage Status](https://coveralls.io/repos/github/wboayue/realflight-bridge/badge.svg?branch=main)](https://coveralls.io/github/wboayue/realflight-bridge?branch=main)

## Overview

[RealFlight](https://www.realflight.com/) is a leading RC flight simulator that provides a realistic, physics-based environment for flying fixed-wing aircraft, helicopters, and drones. Used by both hobbyists and professionals, it simulates aerodynamics, wind conditions, and control responses, making it an excellent tool for flight control algorithm validation.

**RealFlightBridge** is a Rust library that interfaces with [RealFlight Link](https://forums.realflight.com/index.php?threads/flightaxis-link-q-a.32854/), enabling external flight controllers to interact with the simulator. It allows developers to:

* Send control commands to simulated aircraft.
* Receive real-time simulated flight data for state estimation and control.
* Test stabilization and autonomy algorithms in a controlled environment.

## Requirements

- [RealFlight simulator](https://www.realflight.com/) (tested with RealFlight Evolution)
- RealFlight Link enabled: Settings → Physics → Quality → RealFlight Link Enabled. Restart the simulator afterwards.
- Rust 1.85 or later (2024 edition)

## Install

```bash
cargo add realflight-bridge
```

Upgrading from 1.x? 2.0 renames `SimulatorState` fields and changes error variants; see the [CHANGELOG](CHANGELOG.md) migration notes.

## Architecture

RealFlight Link is a SOAP API that requires a new TCP connection per request. Over loopback this is cheap; over a network it is not. The library offers two bridges, each with a sync and an async (`rt-tokio`) variant:

| Bridge | Sync | Async | Use when |
|--------|------|-------|----------|
| Local | `RealFlightLocalBridge` | `AsyncLocalBridge` | Your code runs on the simulator host (recommended) |
| Remote | `RealFlightRemoteBridge` | `AsyncRemoteBridge` | Your code runs elsewhere; talks to `realflight_bridge_proxy` on the simulator host over a compact binary protocol |

## Usage

### Local Connection

Connect to RealFlight Link, reset the aircraft, take over control, and run a control loop:

```rust
use std::error::Error;

use realflight_bridge::{ControlInputs, RealFlightBridge, RealFlightLocalBridge};

fn main() -> Result<(), Box<dyn Error>> {
    // Creates bridge with default configuration (connects to 127.0.0.1:18083)
    let bridge = RealFlightLocalBridge::new()?;

    // Reset the simulation to start from a known state
    bridge.reset_aircraft()?;

    // Disable RC input and enable external control
    bridge.disable_rc()?;

    // Initialize control inputs (12 channels available)
    let mut controls = ControlInputs::default();

    for _ in 0..10_000 {
        // Send control inputs and receive simulator state
        let state = bridge.exchange_data(&controls)?;

        // Update control values based on state...
        controls.channels[2] = if state.altitude_agl_m < 10.0 { 1.0 } else { 0.5 };
    }

    // Hand control back to the RC transmitter
    bridge.enable_rc()?;

    Ok(())
}
```

By default each request opens its own connection (`pool_size: 0`); the simulator isn't contacted until the first request. A non-zero `pool_size` pre-opens connections to hide connect latency, but newer RealFlight versions stall while a pre-opened connection sits idle.

To use a non-default address, timeout, or pool size, pass a `Configuration`:

```rust
use std::time::Duration;

use realflight_bridge::{Configuration, RealFlightLocalBridge};

let config = Configuration {
    simulator_host: "192.168.1.100:18083".to_string(),
    connect_timeout: Duration::from_millis(50),
    ..Default::default()
};
let bridge = RealFlightLocalBridge::with_configuration(&config)?;
```

### Remote Connection

Use the remote bridge when your code cannot run on the simulator host, e.g. developing on a Mac while RealFlight runs on Windows. The proxy forwards requests to the simulator over loopback.

A low-latency link is still required. Wired networks and a Mac talking to RealFlight in a Parallels VM work well; WiFi did not reach the 200 Hz loop rate a flight controller typically needs.

#### Proxy (simulator host)

```bash
cargo install realflight-bridge --features rt-tokio
realflight_bridge_proxy                     # binds 0.0.0.0:8080
realflight_bridge_proxy --bind-address 127.0.0.1:9000
realflight_bridge_proxy --preconnect        # pre-open sim connections (older RealFlight only)
```

The proxy connects to the simulator only while a client is connected, opening a new connection per request. `--preconnect` hides connect latency by keeping the next connection open, but newer RealFlight versions stall while that connection sits idle.

The proxy has no authentication; anyone who can reach the port can control the simulator. Run it only on trusted networks.

The proxy and client must use the same major version of `realflight-bridge`; the wire protocol is not compatible across major versions.

#### Client

```rust
use std::error::Error;

use realflight_bridge::{ControlInputs, RealFlightBridge, RealFlightRemoteBridge};

fn main() -> Result<(), Box<dyn Error>> {
    let client = RealFlightRemoteBridge::new("192.168.12.253:8080")?;

    // Disable RC input and enable external control
    client.disable_rc()?;

    // Send control inputs and receive simulator state
    let controls = ControlInputs::default();
    let state = client.exchange_data(&controls)?;
    println!("AGL {:.1} m", state.altitude_agl_m);

    Ok(())
}
```

### Async

Async bridges are available via the `rt-tokio` feature:

```bash
cargo add realflight-bridge --features rt-tokio
```

```rust
use std::error::Error;
use std::time::Duration;

use realflight_bridge::{AsyncBridge, AsyncLocalBridge, ControlInputs};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Or AsyncLocalBridge::new().await? for defaults
    let bridge = AsyncLocalBridge::builder()
        .connect_timeout(Duration::from_millis(10))
        .build()
        .await?;

    bridge.reset_aircraft().await?;
    bridge.disable_rc().await?;

    let mut controls = ControlInputs::default();
    for _ in 0..10_000 {
        let state = bridge.exchange_data(&controls).await?;
        controls.channels[2] = if state.altitude_agl_m < 10.0 { 1.0 } else { 0.5 };
    }

    bridge.enable_rc().await?;
    Ok(())
}
```

`AsyncRemoteBridge::new("192.168.12.253:8080").await?` is the async counterpart of `RealFlightRemoteBridge`.

### Examples

```bash
cargo run --example smoke_test -- --simulator_host 127.0.0.1:18083
cargo run --example remote_bridge -- --proxy-host 192.168.12.253:8080
```

## Control Channels

`ControlInputs` provides 12 channels. Each value ranges from 0.0 to 1.0:

* 0.0: minimum
* 0.5: neutral/center (control surfaces)
* 1.0: maximum

Which function each channel drives (aileron, elevator, throttle, rudder, ...) depends on the aircraft's channel mapping in RealFlight. The examples assume throttle on channel 3 (`channels[2]`).

## SimulatorState

`SimulatorState` provides flight data as reported by RealFlight. Values are `f32` and passed through unconverted. Field names carry their unit as a suffix:

| Suffix | Unit | Examples |
|--------|------|----------|
| `_m` | meters | `altitude_asl_m`, `altitude_agl_m`, `aircraft_position_x_m` |
| `_mps` | m/s | `airspeed_mps`, `groundspeed_mps`, `velocity_world_mps`, `velocity_body_mps`, `wind_mps` |
| `_mps2` | m/s² | `acceleration_world_mps2`, `acceleration_body_mps2` |
| `_deg` | degrees | `azimuth_deg`, `inclination_deg`, `roll_deg` |
| `_dps` | deg/s | `pitch_rate_dps`, `roll_rate_dps`, `yaw_rate_dps` |
| `_v`, `_a`, `_mah` | volts, amps, mAh | `battery_voltage_v`, `battery_current_draw_a`, `battery_remaining_capacity_mah` |
| `_oz` | US fl oz | `fuel_remaining_oz` |
| `_s` | seconds | `current_physics_time_s` |

Vector quantities are grouped as `Vector3 { x, y, z }` and orientation as `Quaternion { x, y, z, w }`; both convert to and from arrays. Also included: engine RPM, engine/ground-contact/lock flags, and the aircraft status message.

RealFlight does not document its axes, and they differ between fields. Values are passed through unchanged; the `SimulatorState` docs give each field's frame, following ArduPilot's RealFlight integration. In short: `velocity_world_mps` is north/east/down, position and wind are east/north, body acceleration is forward/right/down, and yaw rate is positive nose-left. `velocity_body_mps` and `acceleration_world_mps2` axes are unverified.

```rust
let state = bridge.exchange_data(&controls)?;
println!(
    "AGL {:.1} m, airspeed {:.1} m/s, climb {:.1} m/s",
    state.altitude_agl_m, state.airspeed_mps, -state.velocity_world_mps.z
);
```

Local bridges (`RealFlightLocalBridge`, `AsyncLocalBridge`) provide a `statistics()` method for performance monitoring (request count, error count, frame rate).

## Math Library Interop

The `mint` feature converts `Vector3` and `Quaternion` to and from [`mint`](https://crates.io/crates/mint) types, which nalgebra, glam, cgmath and others accept:

```bash
cargo add realflight-bridge --features mint
```

```rust
use realflight_bridge::Quaternion;

let v: mint::Vector3<f32> = state.velocity_world_mps.into();
let v: nalgebra::Vector3<f32> = v.into();

// orientation is in RealFlight's convention; remap to body-to-NED first, as ArduPilot does
let o = state.orientation;
let ned = Quaternion { x: o.y, y: o.x, z: -o.z, w: o.w };
let q: mint::Quaternion<f32> = ned.into();
let attitude = nalgebra::UnitQuaternion::from_quaternion(q.into());
```

Conversions copy components only and never change frames (see `SimulatorState` docs).

## Sources

The following sources were useful in understanding the RealFlight Link SOAP API:

* RealFlight [developer forums](https://forums.realflight.com/index.php?threads/flightaxis-link-q-a.32854/)
* ArduPilot RealFlight SITL: [SIM_FlightAxis.h](https://github.com/ArduPilot/ardupilot/blob/master/libraries/SITL/SIM_FlightAxis.h), [SIM_FlightAxis.cpp](https://github.com/ArduPilot/ardupilot/blob/master/libraries/SITL/SIM_FlightAxis.cpp)
* Python [Flight Axis implementation](https://github.com/camdeno/F16Capstone/blob/main/FlightAxis/flightaxis.py)

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
