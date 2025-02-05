use realflight_bridge::tests::soap_stub::Server;
use realflight_bridge::{Configuration, RealFlightBridge};
use std::time::Duration;

fn create_configuration(port: u16) -> Configuration {
    Configuration {
        simulator_url: format!("127.0.0.1:{}", port),
        connect_timeout: Duration::from_millis(100),
        buffer_size: 1,
        ..Default::default()
    }
}

fn create_bridge(port: u16) -> RealFlightBridge {
    let configuration = create_configuration(port);
    RealFlightBridge::new(configuration).unwrap()
}

fn main() {
    let port = 18083;

    let mut server = Server::new(port, vec!["reset-aircraft-200".to_string()]);
    server.setup();

    let configuration = create_configuration(port);
    let bridge = RealFlightBridge::new(configuration).unwrap();

    let result = bridge.reset_aircraft();
    println!("result: {:?}", result);
}