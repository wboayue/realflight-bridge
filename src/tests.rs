//! Test utilities and shared test infrastructure.
//!
//! This module provides common test utilities used across the crate.
//! The actual tests are colocated with their respective modules:
//! - `bridge::local::tests` - RealFlightLocalBridge tests
//! - `bridge::remote::tests` - RealFlightRemoteBridge tests
//! - `decoders::tests` - XML decoder tests

#[cfg(test)]
pub mod soap_stub;
