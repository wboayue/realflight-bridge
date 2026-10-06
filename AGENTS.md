# AGENTS.md

This file provides guidance to coding agents when working with code in this repository.

## Build & Test Commands

```bash
cargo build                              # Build project
cargo test                               # Run all tests
cargo test <test_name>                   # Run single test
cargo test --features rt-tokio           # Run tests with async support
cargo bench --features bench-internals   # Run benchmarks
cargo fmt                                # Format code
cargo clippy                             # Run lints
cargo tarpaulin -o html                  # Generate coverage report
```

Use `just` for common tasks: `just build`, `just test`, `just bench`, `just cover`

### Proxy Binary

```bash
cargo install realflight-bridge --features rt-tokio  # Install proxy (requires rt-tokio)
realflight_bridge_proxy                              # Run proxy (default: 0.0.0.0:8080)
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
- **Proxy Server** (internal): Async server that forwards remote requests to local simulator. Used by `realflight_bridge_proxy` binary

**Why proxy exists**: SOAP requires new TCP connection per request, causing significant overhead on non-local connections. The proxy runs locally with the simulator and exposes an efficient binary protocol for remote clients.

### Key Data Types

- `ControlInputs`: 12-channel RC input array (values 0.0-1.0)
- `SimulatorState`: Complete flight state (position, orientation, velocities, accelerations). Fields carry unit suffixes (`_m`, `_mps`, `_deg`, ...); vectors grouped as `Vector3`, orientation as `Quaternion`
- `Configuration`: Connection settings (host, timeout, pool size)
- `StatisticsEngine`: Tracks request count, errors, frame rate for performance monitoring

### Feature Flags

- `rt-tokio`: Async bridge implementations
- `bench-internals`: Expose internal functions for benchmarking

## Conventions

- Async implementations in `async_impl.rs` files alongside sync versions (local, remote bridges)

## Testing Guidelines

### Layout

- Tests always live in a separate file, never inline. Declare with `#[cfg(test)] mod tests;`
  - `foo/mod.rs` -> `foo/tests.rs`
  - `foo.rs` -> `foo/tests.rs` (e.g. `bridge/local/async_impl.rs` -> `bridge/local/async_impl/tests.rs`)
  - Crate-root (`lib.rs`) tests -> `src/tests.rs`
- No `tests/` integration dir; all tests are unit tests inside the crate so they can reach `pub(crate)` items
- Group related tests in submodules named after the unit under test (`mod parse_status_line { ... }`), no `_tests` suffix
- Test fn names: descriptive snake_case behavior, no `test_` prefix (`returns_error_when_no_responses`)
- Shared fixtures used within one test file go in a local `mod fixtures`

### Helpers and stubs

- Shared helpers go in a `#[cfg(test)] pub(crate) mod test_support;` of the owning component. Don't add helper modules elsewhere
  - `soap_client::test_support::Server`: TCP server replaying canned SOAP responses
  - `bridge::wire::test_support`: `MockProxy`, `send`/`recv`/`write_raw_frame` for remote/proxy protocol tests
- `soap_client::stub::StubSoapClient` + `RealFlightLocalBridge::stub()`: sync local bridge without network
- `soap_client::stub_async::AsyncStubSoapClient`: queued-response async SOAP stub
- Test-only items in non-test modules use `#[cfg(test)]`; don't repeat it on items inside an already test-gated module
- Canned simulator responses live in `testdata/responses/{action}-{status}.xml` (e.g. `return-data-200.xml`). Stubs pick the file by key; reference from tests via `include_str!` relative path or `env!("CARGO_MANIFEST_DIR")`

### Writing tests

- Never require a running simulator; use stubs or `test_support::Server`
- Bind test servers to `127.0.0.1:0` to get a free port; never hardcode ports
- Sync and async bridges share behavior; keep their tests in sync but avoid duplicating coverage that the shared code (`ops`, `wire`) already tests
- Async tests use `#[tokio::test]` and are compiled only with `rt-tokio`; run `cargo test` and `cargo test --features rt-tokio` before committing
- Float comparisons use `approx::assert_relative_eq!`
