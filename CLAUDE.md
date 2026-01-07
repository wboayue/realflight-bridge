# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands

```bash
cargo build                              # Build project
cargo test                               # Run all tests
cargo test <test_name>                   # Run single test
cargo test --features rt-tokio           # Run tests with async support
cargo test --features uom                # Run tests with SI units
cargo bench --features bench-internals   # Run benchmarks
cargo fmt                                # Format code
cargo clippy                             # Run lints
cargo tarpaulin -o html                  # Generate coverage report
```

Use `just` for common tasks: `just build`, `just test`, `just bench`, `just cover`

### Proxy Binary

```bash
cargo install realflight-bridge          # Install proxy
realflight_bridge_proxy                  # Run proxy (default: 0.0.0.0:8080)
realflight_bridge_proxy --bind-address <addr>
```

## Architecture

Rust 2024 edition library providing SOAP-based communication with RealFlight Link simulator API.

### Core Traits

- **`RealFlightBridge`**: Sync interface with `exchange_data`, `enable_rc`, `disable_rc`, `reset_aircraft`
- **`AsyncBridge`**: Async version (requires `rt-tokio` feature)

### Bridge Implementations

- **`RealFlightLocalBridge`**: Direct SOAP/TCP connection to simulator. Uses connection pooling. Default: `127.0.0.1:18083`
- **`RealFlightRemoteBridge`**: Connects to proxy using postcard-serialized binary protocol
- **`ProxyServer`**: Forwards remote requests to local simulator

**Why proxy exists**: SOAP requires new TCP connection per request, causing significant overhead on non-local connections. The proxy runs locally with the simulator and exposes an efficient binary protocol for remote clients.

### Key Data Types

- `ControlInputs`: 12-channel RC input array (values 0.0-1.0)
- `SimulatorState`: Complete flight state (position, orientation, velocities, accelerations)
- `Configuration`: Connection settings (host, timeout, pool size)
- `StatisticsEngine`: Tracks request count, errors, frame rate for performance monitoring

### Feature Flags

- `uom`: Strongly-typed SI units via `uom` crate
- `rt-tokio`: Async bridge implementations
- `bench-internals`: Expose internal functions for benchmarking

## Conventions

- Tests in `mod tests` within source files or `<module>/tests.rs`
- Use `#[serial_test::serial]` for tests requiring exclusive simulator access
- Async implementations mirror sync API in `async_impl.rs` files
- Test stubs: `StubSoapClient` and `new_stubbed()` methods enable testing without real simulator
