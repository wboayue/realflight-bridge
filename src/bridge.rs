use crate::{BridgeError, ControlInputs, SimulatorState};

pub mod local;
pub mod proxy;
pub mod remote;

pub trait RealFlightBridge {
    /// Exchanges flight control data with the RealFlight simulator.
    ///
    /// This method transmits the provided [ControlInputs] (e.g., RC channel values)
    /// to the RealFlight simulator and retrieves an updated [SimulatorState] in return,
    /// including position, orientation, velocities, and more.
    ///
    /// # Parameters
    ///
    /// - `control`: A [ControlInputs] struct specifying up to 12 RC channels (0.0–1.0 range).
    ///
    /// # Returns
    ///
    /// A [Result] with the updated [SimulatorState] on success, or an error if
    /// something goes wrong (e.g., SOAP fault, network timeout).
    fn exchange_data(&self, control: &ControlInputs) -> Result<SimulatorState, BridgeError>;

    /// Reverts the RealFlight simulator to use its original Spektrum (or built-in) RC input.
    ///
    /// Calling [RealFlightBridge::enable_rc] instructs RealFlight to restore its native RC controller
    /// device (e.g., Spektrum). Once enabled, external RC control via the RealFlight Link
    /// interface is disabled until you explicitly call [RealFlightBridge::disable_rc].
    ///
    /// # Returns
    ///
    /// `Ok(())` if the simulator successfully reverts to using the original RC controller.
    /// An `Err`` is returned if RealFlight cannot locate or restore the original controller device.
    fn enable_rc(&self) -> Result<(), BridgeError>;

    /// Switches the RealFlight simulator’s input to the external RealFlight Link controller,
    /// effectively disabling any native Spektrum (or other built-in) RC device.
    ///
    /// Once [RealFlightBridge::disable_rc] is called, RealFlight listens exclusively for commands sent
    /// through this external interface. To revert to the original RC device, call
    /// [RealFlightBridge::enable_rc].
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if RealFlight Link mode is successfully activated, or an `Err` if
    /// the request fails (e.g., simulator is not ready or rejects the command).
    fn disable_rc(&self) -> Result<(), BridgeError>;

    /// Resets the currently loaded aircraft in the RealFlight simulator, analogous
    /// to pressing the spacebar in the simulator’s interface.
    ///
    /// This call repositions the aircraft back to its initial state and orientation,
    /// clearing any damage or off-runway positioning. It’s useful for rapid iteration
    /// when testing control loops or flight maneuvers.
    ///
    /// # Returns
    ///
    /// `Ok(())` upon a successful reset. Returns an error if RealFlight rejects the command
    /// or if a network issue prevents delivery.
    fn reset_aircraft(&self) -> Result<(), BridgeError>;
}
