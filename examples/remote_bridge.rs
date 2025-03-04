use realflight_bridge::RealFlightRemoteBridge;

fn main() -> std::io::Result<()> {
    // Connect to the server
    let mut client = RealFlightRemoteBridge::new("127.0.0.1:8080")?;
    println!("Connected to server at 127.0.0.1:8080");

    Ok(())
}