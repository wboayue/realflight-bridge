//! Default settings shared by sync and async implementations.

use std::time::Duration;

/// Timeout for each TCP connect to the simulator. An upper bound only; loopback
/// connects normally take well under a millisecond. With on-demand connections a
/// timeout fails the request, so leave headroom for a loaded host.
pub(crate) const CONNECT_TIMEOUT: Duration = Duration::from_millis(50);

/// Time allowed for the connection pool to become ready.
pub(crate) const INIT_TIMEOUT: Duration = Duration::from_secs(5);

/// 0 connects on demand. Newer RealFlight versions stall while a pre-created
/// connection sits idle, so pre-connecting is opt-in.
pub(crate) const POOL_SIZE: usize = 0;

/// Timeout for connecting to a remote proxy.
pub(crate) const REMOTE_TIMEOUT: Duration = Duration::from_secs(5);
