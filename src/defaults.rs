//! Default settings shared by sync and async implementations.

use std::time::Duration;

/// Timeout for each TCP connect to the simulator. An upper bound only; loopback
/// connects normally take well under a millisecond. With on-demand connections a
/// timeout fails the request, so leave headroom for a loaded host.
pub(crate) const CONNECT_TIMEOUT: Duration = Duration::from_millis(10);

/// Time allowed for the connection pool to become ready.
pub(crate) const INIT_TIMEOUT: Duration = Duration::from_secs(5);

/// 0 connects on demand. Newer RealFlight versions stall while a pre-created
/// connection sits idle, so pre-connecting is opt-in.
pub(crate) const POOL_SIZE: usize = 0;

/// Timeout for connecting to a remote proxy.
pub(crate) const REMOTE_TIMEOUT: Duration = Duration::from_secs(5);

/// Read/write timeout for each remote proxy call. Keep it above the proxy's
/// [`REQUEST_TIMEOUT`] so a stalled simulator surfaces as an error reply rather
/// than a client timeout, which forces a reconnect.
pub(crate) const REMOTE_IO_TIMEOUT: Duration = Duration::from_millis(250);

/// Time allowed for one SOAP request to the simulator, from getting a
/// connection to reading the full response.
pub(crate) const REQUEST_TIMEOUT: Duration = Duration::from_millis(150);
