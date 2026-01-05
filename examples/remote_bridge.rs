use std::error::Error;
use std::time::{Duration, Instant};

use clap::{Command, arg};
use realflight_bridge::{ControlInputs, RealFlightBridge, RealFlightRemoteBridge};

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let matches = Command::new("remote_bridge")
        .about("connects to a realflight_bridge_proxy running on a remote machine")
        .arg(
            arg!(--"proxy-host" <VALUE>)
                .help("host and port to realflight_bridge_proxy. e.g. 196.192.29.74:8080")
                .required(true),
        )
        .get_matches();

    let proxy_host = matches.get_one::<String>("proxy-host").unwrap();
    println!("Connecting to RealFlight bridge proxy at {}", proxy_host);

    let bridge = RealFlightRemoteBridge::new(proxy_host)?;
    println!("Connected to server at {}", proxy_host);

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

    let start = Instant::now();
    let loop_duration = Duration::from_secs(10);

    println!("Starting simulation loop for {:?}", loop_duration);

    let mut i = 0;
    while start.elapsed() < loop_duration {
        // Send control inputs and receive simulator state
        let _ = bridge.exchange_data(&controls)?;

        // Example: set channels to 80% throttle
        controls.channels[0] = 0.8;
        controls.channels[1] = 0.8;
        controls.channels[2] = 0.8;
        controls.channels[3] = 0.8;

        i += 1;
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
