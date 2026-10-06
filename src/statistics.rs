//! Statistics tracking for RealFlight bridge operations.

use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

/// Represents a snapshot of performance metrics for a running `RealFlightBridge`.
///
/// The `Statistics` struct is returned by [`RealFlightLocalBridge::statistics`](crate::RealFlightLocalBridge::statistics)
/// and captures various counters and timings that can help diagnose performance issues
/// or monitor real-time operation.
///
/// # Fields
///
/// - `runtime`: The total elapsed time since the `RealFlightBridge` instance was created.
/// - `error_count`: The number of errors (e.g., connection errors, SOAP faults) encountered so far.
/// - `frequency`: An approximate request rate, calculated as `(request_count / runtime)`.
/// - `request_count`: The total number of SOAP requests sent to the simulator. Loops back to 0 after `u32::MAX`.
///
/// ```no_run
/// use realflight_bridge::{RealFlightLocalBridge, BridgeError};
///
/// fn main() -> Result<(), BridgeError> {
///     let bridge = RealFlightLocalBridge::new()?;
///
///     // Send some commands...
///
///     // Now retrieve statistics to assess performance
///     let stats = bridge.statistics();
///     println!("Runtime: {:?}", stats.runtime);
///     println!("Frequency: {:.2} Hz", stats.frequency);
///     println!("Errors so far: {}", stats.error_count);
///
///     Ok(())
/// }
/// ```
///
/// This information can help identify connection bottlenecks, excessive errors,
/// or confirm that a high-frequency control loop is operating as expected.
#[derive(Debug)]
pub struct Statistics {
    pub runtime: Duration,
    pub error_count: u32,
    pub frequency: f32,
    pub request_count: u32,
}

/// Statistics engine for tracking bridge operations.
pub(crate) struct StatisticsEngine {
    start_time: Instant,
    error_count: AtomicU32,
    request_count: AtomicU32,
}

impl StatisticsEngine {
    pub fn new() -> Self {
        StatisticsEngine {
            start_time: Instant::now(),
            error_count: AtomicU32::new(0),
            request_count: AtomicU32::new(0),
        }
    }

    pub fn snapshot(&self) -> Statistics {
        Statistics {
            runtime: self.start_time.elapsed(),
            error_count: self.error_count(),
            frequency: self.frame_rate(),
            request_count: self.request_count(),
        }
    }

    fn error_count(&self) -> u32 {
        self.error_count.load(Ordering::Relaxed)
    }

    fn request_count(&self) -> u32 {
        self.request_count.load(Ordering::Relaxed)
    }

    pub(crate) fn increment_request_count(&self) {
        self.request_count.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn increment_error_count(&self) {
        self.error_count.fetch_add(1, Ordering::Relaxed);
    }

    fn frame_rate(&self) -> f32 {
        self.request_count() as f32 / self.start_time.elapsed().as_secs_f32()
    }
}

impl Default for StatisticsEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
