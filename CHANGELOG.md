# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Removed
- **Breaking:** `uom` feature and dependency. `SimulatorState` fields are now always `f32` ([#52](https://github.com/wboayue/realflight-bridge/issues/52))

### Changed
- Documented the unit of every `SimulatorState` field

### Migration
- Replace `state.field.get::<unit>()` with `state.field`. Values are in the units RealFlight reports (see field docs).
- `fuel_remaining` is in ounces (was liters with `uom`); divide by 33.814 for liters.
- Angles and angular rates are in degrees (`uom` stored radians); use `.to_radians()` if needed.

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
