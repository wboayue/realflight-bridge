use realflight_bridge::bridge::remote::ProxyServer;

// Proxy server that listens on
// Only one active connection is supported
fn main() -> std::io::Result<()> {
    let mut server = ProxyServer::new(80);
    server.run()?;

    Ok(())
}
