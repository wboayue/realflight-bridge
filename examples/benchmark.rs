use std::time::Instant;

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

    let mut client = match RealFlightLink::connect(simulator_url) {
        Ok(client) => client,
        Err(e) => {
            eprintln!("Error connecting to RealFlight simulator: {}", e);
            std::process::exit(1);
        }
    };

    client.reset_sim()?;
//    client.disable_rc()?;

    let start_time = Instant::now();

    let count = 200;
    let mut control = ControlInputs::default();
    for i in 0..12 {
        control.channels[i] = 1.0;
    }
    for i in 0..count {
//        control.channels[i] = 1.0;
        let state = client.exchange_data(&control)?;
//        println!("state: {:?}", state);
    }

    let elapsed_time = start_time.elapsed();
    println!("Time taken: {:?}", elapsed_time);
    println!("RPS: {:?}", count as f64 / elapsed_time.as_secs_f64());

    Ok(())
}
