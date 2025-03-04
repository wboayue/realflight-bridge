use std::time::{Duration, Instant};
use std::{error::Error, thread};

use realflight_bridge::{ControlInputs, RealFlightRemoteBridge};

fn main() -> Result<(), Box<dyn Error>> {
    // Connect to the server
//    let host = "127.0.0.1:8080";
    let host = "192.168.4.117:8080";
    let mut bridge = RealFlightRemoteBridge::new(host)?;
    println!("Connected to server at {}", host);

    // target frequency

    // Reset the simulation to start from a known state
    if let Err(e) = bridge.reset_aircraft() {
        eprintln!("Error resetting aircraft: {}", e);
    }

    // Disable RC input and enable external control
    if let Err(e) = bridge.disable_rc() {
        eprintln!("Error disabling RC control: {}", e);
    }

    // Initialize control inputs (12 channels available)
    let mut controls: ControlInputs = ControlInputs::default();

    let tick_duration = Duration::from_secs(1) / 300;

    let start = Instant::now();
    let loop_duration = Duration::from_secs(20);

    println!("Starting simulation loop for {:?}", loop_duration);

    let mut i = 0;
    while start.elapsed() < loop_duration {
        let tick_start = Instant::now();

        // Send control inputs and receive simulator state
        // let _ = bridge.exchange_data(&controls)?;
        let _ = bridge.reset_aircraft()?;

        let output = ((i as f32 / 1000.0).sin() + 1.0) / 2.0;

        // Update control values based on state...
        controls.channels[0] = output; // Example: set first channel to 50%
        controls.channels[1] = 0.5; // Example: set first channel to 50%
        controls.channels[2] = output; // Example: set first channel to 50%
        controls.channels[3] = 0.5; // Example: set first channel to 50%

        i += 1;
//        thread::sleep(tick_duration - tick_start.elapsed());
    }

    println!("Simulation complete: {} cycles in {:?}", i, start.elapsed());
    println!(
        "Achieved rate: {:.1} Hz",
        i as f32 / start.elapsed().as_secs_f32()
    );

    if let Err(e) = bridge.enable_rc() {
        eprintln!("Error restoring RC control: {}", e);
    }

    Ok(())
}
