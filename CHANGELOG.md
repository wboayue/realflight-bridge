# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed
- Simulator stalls on newer RealFlight versions, which hang while a pre-opened SOAP connection sits idle:
  - Default `pool_size` is 0 (sync and async): each request opens its own connection. Bridge construction no longer contacts the simulator; an unreachable simulator is reported by the first request
  - Proxy connects to the simulator only while a client is connected; it starts without RealFlight running
  - Async pool held `pool_size + 1` idle connections; now at most `pool_size`
  - Default `connect_timeout` raised from 5ms to 10ms, since a connect timeout now fails the request instead of a background retry
- `pool_size: 0` panicked in the async pool; it now connects on demand

### Added
- Proxy `--preconnect` flag / `AsyncProxyServer::preconnect(bool)` to opt into pre-opened connections on RealFlight versions that tolerate them
- `AsyncProxyServer::simulator_addr(SocketAddr)` to forward to a non-default simulator address

## [2.0.0] - 2026-10-06

### Added
- `Vector3` and `Quaternion` types, with `[f32; 3]` / `[f32; 4]` conversions
- `mint` feature: `Vector3` / `Quaternion` convert to and from `mint` types, for interop with nalgebra, glam, cgmath, etc. ([#53](https://github.com/wboayue/realflight-bridge/issues/53))
- `Clone` for `SimulatorState`
- Coordinate frame documentation for `SimulatorState` fields
- `BridgeError::Protocol` for malformed or unexpected responses from the simulator or proxy ([#60](https://github.com/wboayue/realflight-bridge/issues/60))
- Remote bridges return the proxy-side error (e.g. a simulator `SoapFault`) instead of a generic failure. Relayed `Connection` / `Initialization` messages are prefixed with `proxy: `
- Remote frames are capped at 64 KiB. Oversized frames return `BridgeError::Connection` (`InvalidData`); the proxy drops clients that send one ([#56](https://github.com/wboayue/realflight-bridge/issues/56))

### Changed
- **Breaking:** `SimulatorState` physical fields renamed with unit suffixes (e.g. `airspeed` → `airspeed_mps`, `altitude_agl` → `altitude_agl_m`, `azimuth` → `azimuth_deg`, `pitch_rate` → `pitch_rate_dps`, `fuel_remaining` → `fuel_remaining_oz`, `current_physics_time` → `current_physics_time_s`)
- **Breaking:** vector fields grouped into `Vector3` (`velocity_world_mps`, `velocity_body_mps`, `acceleration_world_mps2`, `acceleration_body_mps2`, `wind_mps`) and quaternion fields into `orientation: Quaternion`. RealFlight u/v/w components map to x/y/z
- **Breaking:** `BridgeError` is `#[non_exhaustive]`
- **Breaking:** HTTP parsing and remote protocol errors return `BridgeError::Protocol` instead of `SoapFault`. `SoapFault` now only means the simulator returned a fault
- **Breaking:** wire format: `ResponseStatus::Error` carries a `RemoteError`. Proxy and clients must both be 2.x
- Documented the unit of every `SimulatorState` field
- Decoder reads leaf elements that carry XML attributes (previously skipped)
- Decoder ~33% faster: tokenizer reuses buffers instead of allocating per tag
- Remote bridges and proxy reuse frame encode buffers instead of allocating per request ([#61](https://github.com/wboayue/realflight-bridge/issues/61))
- `RealFlightRemoteBridge` returns `BridgeError::Connection` after a call fails mid-exchange instead of reading out-of-sync data; create a new bridge to recover
- Proxy replies with `BridgeError::Protocol` when a response exceeds the frame limit, instead of dropping the client
- Local bridges' `Statistics::request_count` counts every attempted request, including ones that fail to send (previously only requests written successfully)
- Local bridges' `Statistics::error_count` counts failed requests (I/O errors, SOAP faults, parse errors), as documented. Previously only failed background connection attempts were counted

### Removed
- **Breaking:** `uom` feature and dependency. `SimulatorState` fields are now always `f32` ([#52](https://github.com/wboayue/realflight-bridge/issues/52))

### Fixed
- Decoder returns a `Parse` error instead of panicking when a response has more than 12 channel values
- `aircraft_position_x_m` / `aircraft_position_y_m` docs: X is east, Y is north (previously documented as north/east)

### Migration
- Rename field accesses to their suffixed names; e.g. `velocity_world_u` → `velocity_world_mps.x`, `orientation_quaternion_w` → `orientation.w`. `uom` users: replace `state.field.get::<unit>()` with `state.field_<unit>`.
- `fuel_remaining_oz` is in US fluid ounces (was liters with `uom`); 1 L = 33.814 US fl oz.
- Angles and angular rates are in degrees (`uom` stored radians); use `.to_radians()` if needed.
- Add a `_ =>` arm when matching `BridgeError`. Match `Protocol` where you previously matched non-fault `SoapFault` messages (e.g. "Missing Content-Length header", "Proxy reported operation failure").
- Upgrade the proxy and remote clients together.

## [1.1.0] - 2026-07-22

### Changed
- Updated `uom` requirement from 0.37.0 to 0.38.0
- Updated `rand` requirement from 0.9 to 0.10

## [1.0.0] - 2026-01-11

### Added
- `AsyncBridge` trait for Tokio-based async operations
- `AsyncLocalBridge` and `AsyncRemoteBridge` implementations
- `AsyncLocalBridgeBuilder` and `AsyncRemoteBridgeBuilder` for custom configuration
- `AsyncProxyServer` for remote simulator access
- `Statistics` API for performance monitoring (request count, error count, frequency)
- `BridgeError` custom error type with structured variants
- Connection timeout configuration for `RealFlightRemoteBridge`
- `rt-tokio` feature flag for async support

### Changed
- **Breaking:** Proxy server now async-only (requires `rt-tokio` feature)
- **Breaking:** Module structure reorganized following Single Responsibility Principle
- Upgraded to Rust 2024 edition
- Replaced panics with proper error propagation

### Improved
- Test coverage increased to >90%
- Reduced allocations in hot paths
- Connection pooling for SOAP requests

## [0.1.0 - 0.5.0] - 2024

Pre-1.0 development releases. Key milestones:
- `RealFlightBridge` trait and `RealFlightLocalBridge` implementation
- SOAP/TCP communication with RealFlight Link
- Full `SimulatorState` parsing (45+ fields)
- `RealFlightRemoteBridge` and proxy server binary
- Connection pooling for improved performance
- `uom` feature flag for strongly-typed SI units
- Benchmarking infrastructure
