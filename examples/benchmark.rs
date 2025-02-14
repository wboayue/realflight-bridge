use clap::{arg, Command};
use log::{debug, info};

use realflight_bridge::{Configuration, ControlInputs, RealFlightBridge};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let matches = Command::new("record")
        .version("1.0")
        .author("Wil Boayue <wil@wsbsolutions.com")
        .about("save simulator data to a file")
        .arg(
            arg!(--simulator_url <VALUE>)
                .help("url to RealFlight simulator")
                .default_value("127.0.0.1:18083"),
        )
        .get_matches();

    let simulator_url = matches.get_one::<String>("simulator_url").unwrap();
    info!("Connecting to RealFlight simulator at {}", simulator_url);

    let configuration = Configuration {
        simulator_url: simulator_url.clone(),
        ..Default::default()
    };

    let bridge = match RealFlightBridge::new(configuration) {
        Ok(client) => client,
        Err(e) => {
            eprintln!("Error connecting to RealFlight simulator: {}", e);
            std::process::exit(1);
        }
    };

    bridge.reset_aircraft()?;
    if let Err(e) = bridge.disable_rc() {
        eprintln!("Error disabling RC: {}", e);
    }

    let control = ControlInputs::default();

    for _ in 0..200 {
        let state = bridge.exchange_data(&control)?;
        debug!("state: {:?}", state);
    }

    bridge.enable_rc()?;

    let statistics = bridge.statistics();

    println!("Runtime: {:?}", statistics.runtime);
    println!("Frame Rate: {:?}", statistics.frame_rate);
    println!("Error Count: {:?}", statistics.error_count);
    println!("Request count: {:?}", statistics.request_count);

    Ok(())
}
