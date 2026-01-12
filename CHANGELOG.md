# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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

## [0.5.0] - 2024-12-XX

### Added
- `uom` feature flag for strongly-typed SI units
- Connection pool management
- Remote bridge for proxy connections

### Changed
- Switched numeric fields from f64 to f32

## [0.4.0] - 2024-XX-XX

### Added
- `RealFlightRemoteBridge` for connecting via proxy
- Proxy server binary (`realflight_bridge_proxy`)
- Binary protocol using postcard serialization

## [0.3.0] - 2024-XX-XX

### Added
- Full `SimulatorState` parsing with 45+ fields
- Connection pooling for improved performance
- Benchmarking infrastructure

## [0.2.0] - 2024-XX-XX

### Added
- `RealFlightLocalBridge` implementation
- SOAP/TCP communication with RealFlight Link
- `ControlInputs` for 12-channel RC input
- Basic `SimulatorState` response parsing

## [0.1.0] - 2024-XX-XX

### Added
- Initial release
- `RealFlightBridge` trait definition
- Basic SOAP client skeleton
