//! Wire protocol shared by remote bridges (clients) and the proxy (server):
//! message types, length-prefixed framing, and response interpretation.

use log::error;
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteError {
    /// Which [BridgeError] variant the proxy saw
    pub kind: RemoteErrorKind,
    /// Error message (for `Parse`, the parse message without the field)
    pub message: String,
    /// Field name for `Parse` errors
    pub field: Option<String>,
}

/// [BridgeError] variant carried by a [RemoteError].
///
/// Encoded by index, so proxy and client must use the same major version.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum RemoteErrorKind {
    Connection,
    Initialization,
    SoapFault,
    Parse,
    Protocol,
}

impl RemoteError {
    fn new(kind: RemoteErrorKind, message: String) -> Self {
        Self {
            kind,
            message,
            field: None,
        }
    }
}

impl From<&BridgeError> for RemoteError {
    fn from(err: &BridgeError) -> Self {
        match err {
            BridgeError::Connection(e) => Self::new(RemoteErrorKind::Connection, e.to_string()),
            BridgeError::Initialization(msg) => {
                Self::new(RemoteErrorKind::Initialization, msg.clone())
            }
            BridgeError::SoapFault(msg) => Self::new(RemoteErrorKind::SoapFault, msg.clone()),
            BridgeError::Parse { field, message } => Self {
                kind: RemoteErrorKind::Parse,
                message: message.clone(),
                field: Some(field.clone()),
            },
            BridgeError::Protocol(msg) => Self::new(RemoteErrorKind::Protocol, msg.clone()),
        }
    }
}

impl From<RemoteError> for BridgeError {
    fn from(err: RemoteError) -> Self {
        match err.kind {
            RemoteErrorKind::Connection => {
                BridgeError::Connection(std::io::Error::other(err.message))
            }
            RemoteErrorKind::Initialization => BridgeError::Initialization(err.message),
            RemoteErrorKind::SoapFault => BridgeError::SoapFault(err.message),
            RemoteErrorKind::Parse => BridgeError::Parse {
                field: err.field.unwrap_or_default(),
                message: err.message,
            },
            RemoteErrorKind::Protocol => BridgeError::Protocol(err.message),
        }
    }
}

impl Response {
    /// Extracts the simulator state from an `ExchangeData` response.
    pub(crate) fn into_state(self) -> Result<SimulatorState, BridgeError> {
        let payload = self.payload;
        Self::check_status(self.status)?;
        payload.ok_or_else(|| {
            error!("No payload in response");
            BridgeError::Protocol("No payload in response".to_string())
        })
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
