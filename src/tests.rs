//! Test utilities and shared test infrastructure.
//!
//! This module provides common test utilities used across the crate.
//! The actual tests are colocated with their respective modules:
//! - `bridge::local::tests` - RealFlightLocalBridge tests
//! - `bridge::remote::tests` - RealFlightRemoteBridge tests
//! - `decoders::tests` - XML decoder tests
//! - `type_conversions` - Vector3/Quaternion array conversions

#[cfg(test)]
pub mod soap_stub;

#[cfg(test)]
mod type_conversions {
    use crate::{Quaternion, Vector3};

    #[test]
    fn vector3_round_trips_through_array() {
        let v = Vector3::from([1.0, 2.0, 3.0]);
        assert_eq!(
            v,
            Vector3 {
                x: 1.0,
                y: 2.0,
                z: 3.0
            }
        );
        assert_eq!(<[f32; 3]>::from(v), [1.0, 2.0, 3.0]);
    }

    #[test]
    fn quaternion_round_trips_through_array_scalar_last() {
        let q = Quaternion::from([1.0, 2.0, 3.0, 4.0]);
        assert_eq!(
            q,
            Quaternion {
                x: 1.0,
                y: 2.0,
                z: 3.0,
                w: 4.0
            }
        );
        assert_eq!(<[f32; 4]>::from(q), [1.0, 2.0, 3.0, 4.0]);
    }
}
