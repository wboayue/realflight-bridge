//! Wire protocol shared by remote bridges (clients) and the proxy (server):
//! message types, length-prefixed framing, and response interpretation.

use std::io;

use serde::{Deserialize, Serialize};

use crate::{BridgeError, ControlInputs, SimulatorState};

pub(crate) mod frame;
pub(crate) mod frame_io;
#[cfg(test)]
pub(crate) mod test_support;
#[cfg(test)]
mod tests;

/// Defines the types of requests that can be sent to the server.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub enum RequestType {
    /// Enable remote control
    EnableRC,
    /// Disable remote control (enable control by ReaFlight link)
    DisableRC,
    /// Reset the aircraft state (like pressing space-bar in the simulator)
    ResetAircraft,
    /// Send [ControlInputs] and receive [SimulatorState]
    ExchangeData,
}

/// Represents a request sent from the client to the server.
#[derive(Debug, Serialize, Deserialize)]
pub struct Request {
    /// Type of request being made
    pub request_type: RequestType,
    /// Optional [ControlInputs] data
    pub payload: Option<ControlInputs>,
}

/// Borrowed form of [Request] for sending without cloning the payload.
/// Serializes identically to [Request].
#[derive(Debug, Serialize)]
pub(crate) struct RequestRef<'a> {
    pub request_type: RequestType,
    pub payload: Option<&'a ControlInputs>,
}

/// Represents a response sent from the server to the client.
#[derive(Debug, Serialize, Deserialize)]
pub struct Response {
    /// Indicates success or failure
    pub status: ResponseStatus,
    /// Optional [SimulatorState] data
    pub payload: Option<SimulatorState>,
}

/// Indicates the status of a response.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub enum ResponseStatus {
    /// Operation completed successfully
    Success,
    /// Operation failed; carries the proxy-side error
    Error(RemoteError),
}

/// An error reported by the proxy, sent to the client so it can rebuild the [BridgeError].
///
/// Mirrors [BridgeError]. Encoded by variant index, so proxy and client must use the
/// same major version.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RemoteError {
    /// Proxy could not complete the request with the simulator ([BridgeError::Simulator])
    Simulator { kind: IoKind, message: String },
    /// Proxy failed to initialize its simulator bridge ([BridgeError::Initialization])
    Initialization(String),
    /// Simulator returned a SOAP fault ([BridgeError::SoapFault])
    SoapFault(String),
    /// Proxy failed to parse the simulator response ([BridgeError::Parse])
    Parse { field: String, message: String },
    /// Malformed request or simulator response ([BridgeError::Protocol])
    Protocol(String),
}

/// Defines [IoKind] with one variant per listed [io::ErrorKind] plus `Other`,
/// and conversions both ways; kinds not listed travel as `Other`.
macro_rules! io_kinds {
    ($($kind:ident),+ $(,)?) => {
        /// Wire form of [io::ErrorKind], which isn't serializable.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        pub enum IoKind {
            $($kind,)+
            Other,
        }

        impl From<io::ErrorKind> for IoKind {
            fn from(kind: io::ErrorKind) -> Self {
                match kind {
                    $(io::ErrorKind::$kind => IoKind::$kind,)+
                    _ => IoKind::Other,
                }
            }
        }

        impl From<IoKind> for io::ErrorKind {
            fn from(kind: IoKind) -> Self {
                match kind {
                    $(IoKind::$kind => io::ErrorKind::$kind,)+
                    IoKind::Other => io::ErrorKind::Other,
                }
            }
        }
    };
}

// Variants are encoded by index, so editing this list changes the wire format
io_kinds!(
    TimedOut,
    ConnectionRefused,
    ConnectionReset,
    ConnectionAborted,
    BrokenPipe,
    UnexpectedEof,
);

impl RemoteError {
    /// Simulator failure carrying `err`'s kind and message.
    fn simulator(err: &io::Error) -> Self {
        RemoteError::Simulator {
            kind: err.kind().into(),
            message: err.to_string(),
        }
    }
}

/// Prefix for relayed initialization errors, which would otherwise look like
/// client-side failures.
const PROXY_PREFIX: &str = "proxy: ";

impl From<&BridgeError> for RemoteError {
    fn from(err: &BridgeError) -> Self {
        match err {
            // The proxy's local bridge doesn't return `Connection` for requests;
            // if one appears, it's still not a failure of the client's link
            BridgeError::Connection(e) | BridgeError::Simulator(e) => RemoteError::simulator(e),
            BridgeError::Initialization(msg) => RemoteError::Initialization(msg.clone()),
            BridgeError::SoapFault(msg) => RemoteError::SoapFault(msg.clone()),
            BridgeError::Parse { field, message } => RemoteError::Parse {
                field: field.clone(),
                message: message.clone(),
            },
            BridgeError::Protocol(msg) => RemoteError::Protocol(msg.clone()),
        }
    }
}

/// Rebuilds the proxy-side error; `Initialization` messages are prefixed with
/// `"proxy: "`.
impl From<RemoteError> for BridgeError {
    fn from(err: RemoteError) -> Self {
        match err {
            RemoteError::Simulator { kind, message } => {
                BridgeError::Simulator(io::Error::new(kind.into(), message))
            }
            RemoteError::Initialization(msg) => {
                BridgeError::Initialization(format!("{PROXY_PREFIX}{msg}"))
            }
            RemoteError::SoapFault(msg) => BridgeError::SoapFault(msg),
            RemoteError::Parse { field, message } => BridgeError::Parse { field, message },
            RemoteError::Protocol(msg) => BridgeError::Protocol(msg),
        }
    }
}

impl Response {
    /// Extracts the simulator state from an `ExchangeData` response.
    pub(crate) fn into_state(self) -> Result<SimulatorState, BridgeError> {
        let payload = self.payload;
        Self::check_status(self.status)?;
        payload.ok_or_else(|| BridgeError::Protocol("No payload in response".to_string()))
    }

    /// Interprets a response to an operation without a payload.
    pub(crate) fn into_unit(self) -> Result<(), BridgeError> {
        Self::check_status(self.status)
    }

    /// Rebuilds the proxy-side error from an error status.
    fn check_status(status: ResponseStatus) -> Result<(), BridgeError> {
        match status {
            ResponseStatus::Success => Ok(()),
            ResponseStatus::Error(err) => Err(err.into()),
        }
    }
}

#[cfg(feature = "rt-tokio")]
impl Response {
    pub(crate) fn success() -> Self {
        Self {
            status: ResponseStatus::Success,
            payload: None,
        }
    }

    pub(crate) fn success_with(state: SimulatorState) -> Self {
        Self {
            status: ResponseStatus::Success,
            payload: Some(state),
        }
    }

    pub(crate) fn error(err: &BridgeError) -> Self {
        Self {
            status: ResponseStatus::Error(err.into()),
            payload: None,
        }
    }
}
