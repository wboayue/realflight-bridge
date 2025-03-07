use std::error::Error;

use clap::{arg, Command};
use realflight_bridge::bridge::remote::ProxyServer;

/// Entry point for the RealFlight Bridge Proxy server application.
///
/// This application starts a proxy server that listens for remote connections and forwards them
/// to the RealFlight simulator. The proxy is expected to run on the same machine as the simulator.
///
/// # Command-Line Arguments
///
/// - `--bind_address <VALUE>`: Specifies the network address (IP and port) on which the proxy server
///   will listen for incoming connections. The value should be in the format `IP:PORT` (e.g., `0.0.0.0:8080`).
///   If not provided, it defaults to `0.0.0.0:8080`.
///
/// # Errors
///
/// This function returns a boxed error (`Box<dyn Error>`) if any part of the server initialization or
/// execution fails.
fn main() -> Result<(), Box<dyn Error>> {
    let matches = Command::new("realflight_bridge_proxy")
        .version("0.1")
        .about("Starts a proxy server for the RealFlight bridge")
        .arg(
            arg!(--bind_address <VALUE>)
                .help("Address to bind the server to")
                .default_value("0.0.0.0:8080"),
        )
        .get_matches();

    let bind_address = matches.get_one::<String>("bind_address").unwrap();

    let mut server = ProxyServer::new(bind_address);
    // let mut server = ProxyServer::new_stubbed(bind_address);
    server.run()?;

    Ok(())
}
