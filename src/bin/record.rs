use clap::{arg, Command};
use log::debug;

use realflight_link::{ControlInputs, RealFlightLink};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let matches = Command::new("record")
        .version("1.0")
        .author("Wil Boayue <wil@wsbsolutions.com")
        .about("save simulator data to a file")
        .arg(
            arg!(--simulator_url <VALUE>)
                .help("url to RealFlight simulator")
                .default_value("http://127.0.0.1:18083"),
        )
        .get_matches();

    let simulator_url = matches.get_one::<String>("simulator_url").unwrap();
    debug!("Connecting to RealFlight simulator at {}", simulator_url);

    let client = RealFlightLink::new(simulator_url);

    // Only record data don't send control commands
    client.enable_rc()?;

    let control = ControlInputs::default();
    for i in 0..10 {
        let state = client.exchange_data(&control)?;
        println!("state: {:?}", state);
    }
    Ok(())
}
