//! Request deadlines shared by the sync and async clients.

use std::io::{self, ErrorKind, Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

#[cfg(test)]
mod tests;

/// Error for a request that got no response from `peer` within `after`.
pub(crate) fn timed_out(peer: &str, after: Duration) -> io::Error {
    io::Error::new(
        ErrorKind::TimedOut,
        format!("no response from {peer} within {after:?}"),
    )
}

/// Rejects a zero timeout, which would fail every request.
pub(crate) fn ensure_nonzero(name: &str, timeout: Duration) -> io::Result<()> {
    if timeout.is_zero() {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            format!("{name} must be non-zero"),
        ));
    }
    Ok(())
}

/// Point in time a request must finish by. A timeout too large to represent
/// (e.g. `Duration::MAX`) means no deadline.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Deadline {
    at: Option<Instant>,
    timeout: Duration,
}

impl Deadline {
    /// Deadline `timeout` from now.
    pub(crate) fn after(timeout: Duration) -> Self {
        Deadline {
            at: Instant::now().checked_add(timeout),
            timeout,
        }
    }

    /// No deadline.
    pub(crate) fn none() -> Self {
        Deadline {
            at: None,
            timeout: Duration::MAX,
        }
    }

    /// Time left, `None` if unbounded, or a `TimedOut` error naming `peer`
    /// once the deadline has passed.
    pub(crate) fn remaining(&self, peer: &str) -> io::Result<Option<Duration>> {
        match self.at {
            None => Ok(None),
            Some(at) => {
                let left = at.saturating_duration_since(Instant::now());
                if left.is_zero() {
                    return Err(timed_out(peer, self.timeout));
                }
                Ok(Some(left))
            }
        }
    }
}

/// TCP stream whose reads and writes share one [Deadline]: each call's socket
/// timeout is set to the time left, so a peer trickling bytes can't extend it.
/// Timeouts surface as `TimedOut` errors naming the peer.
pub(crate) struct DeadlineStream {
    stream: TcpStream,
    peer: &'static str,
    deadline: Deadline,
}

impl DeadlineStream {
    /// Wraps `stream` with no deadline; `peer` names it in timeout errors.
    pub(crate) fn new(stream: TcpStream, peer: &'static str) -> Self {
        DeadlineStream {
            stream,
            peer,
            deadline: Deadline::none(),
        }
    }

    /// Sets the deadline for subsequent reads and writes.
    pub(crate) fn set_deadline(&mut self, deadline: Deadline) {
        self.deadline = deadline;
    }

    /// Maps a socket timeout (`WouldBlock` on Unix) to the deadline error.
    fn check(&self, e: io::Error) -> io::Error {
        match e.kind() {
            ErrorKind::WouldBlock | ErrorKind::TimedOut => {
                timed_out(self.peer, self.deadline.timeout)
            }
            _ => e,
        }
    }
}

impl Read for DeadlineStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.stream
            .set_read_timeout(self.deadline.remaining(self.peer)?)?;
        self.stream.read(buf).map_err(|e| self.check(e))
    }
}

impl Write for DeadlineStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.stream
            .set_write_timeout(self.deadline.remaining(self.peer)?)?;
        self.stream.write(buf).map_err(|e| self.check(e))
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }
}
