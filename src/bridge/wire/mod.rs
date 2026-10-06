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
#[derive(Debug, Serialize, Deserialize)]
pub enum ResponseStatus {
    /// Operation completed successfully
    Success,
    /// Operation failed
    Error,
}

impl Response {
    /// Extracts the simulator state from an `ExchangeData` response.
    pub(crate) fn into_state(self) -> Result<SimulatorState, BridgeError> {
        self.check_status()?;
        match self.payload {
            Some(state) => Ok(state),
            None => {
                error!("No payload in response: {:?}", self.status);
                Err(BridgeError::SoapFault("No payload in response".to_string()))
            }
        }
    }

    /// Interprets a response to an operation without a payload.
    pub(crate) fn into_unit(self) -> Result<(), BridgeError> {
        self.check_status()
    }

    /// Maps an error status to a fault. The proxy logs the underlying cause.
    fn check_status(&self) -> Result<(), BridgeError> {
        match self.status {
            ResponseStatus::Success => Ok(()),
            ResponseStatus::Error => Err(BridgeError::SoapFault(
                "Proxy reported operation failure".to_string(),
            )),
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

    pub(crate) fn error() -> Self {
        Self {
            status: ResponseStatus::Error,
            payload: None,
        }
    }
}
