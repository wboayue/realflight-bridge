use std::error::Error;

use realflight_bridge::bridge::remote::ProxyServer;

// Proxy server that listens on
// Only one active connection is supported
fn main() -> Result<(), Box<dyn Error>> {
    let mut server = ProxyServer::new(80)?;
    server.run()?;

    Ok(())
}
