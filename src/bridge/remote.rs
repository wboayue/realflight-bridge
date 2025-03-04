use std::net::TcpStream;

// Client struct to handle the connection and communications
pub struct RealFlightRemoteBridge {
    stream: TcpStream,
    request_counter: u32,
}

impl RealFlightRemoteBridge {
    pub fn new(address: &str) -> std::io::Result<Self> {
        let stream = TcpStream::connect(address)?;
        Ok(RealFlightRemoteBridge {
            stream,
            request_counter: 0,
        })
    }
}
