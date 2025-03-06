use std::io::{self, BufReader, BufWriter, ErrorKind};
use std::net::UdpSocket;
use std::{
    error::Error,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    time::Duration,
};

use postcard::{from_bytes, to_stdvec};
use serde::{Deserialize, Serialize};

use crate::{Configuration, ControlInputs, RealFlightBridge, SimulatorState};

// Define the same structures as in the server
#[derive(Debug, Serialize, Deserialize)]
pub enum RequestType {
    EnableRC,
    DisableRC,
    ResetAircraft,
    ExchangeData,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Request {
    pub request_id: u32,
    pub request_type: RequestType,
    pub payload: Option<ControlInputs>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Response {
    pub request_id: u32,
    pub status: ResponseStatus,
    pub payload: Option<SimulatorState>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ResponseStatus {
    Success,
    Error,
}

// Client struct to handle the connection and communications
pub struct RealFlightRemoteBridge {
    socket: UdpSocket,
    server_address: String,
    request_counter: u32,
}

impl RealFlightRemoteBridge {
    pub fn new(address: &str) -> std::io::Result<Self> {
        let socket = UdpSocket::bind("0.0.0.0:0")?; // Bind to any available port
        socket.set_nonblocking(true)?;

        Ok(RealFlightRemoteBridge {
            socket,
            server_address: address.to_string(),
            request_counter: 0,
        })
    }

    fn send_request(
        &mut self,
        request_type: RequestType,
        payload: Option<ControlInputs>,
    ) -> std::io::Result<Response> {
        // Increment request counter for each new request
        self.request_counter = self.request_counter.wrapping_add(1);

        let request = Request {
            request_id: self.request_counter,
            request_type,
            payload,
        };

        // Serialize the request
        let request_bytes =
            to_stdvec(&request).map_err(|e| io::Error::new(ErrorKind::InvalidData, e))?;

        // Send the request (no length prefix needed since UDP is message-based)
        self.socket.send_to(&request_bytes, &self.server_address)?;

        // Wait for response (with timeout)
        let mut buffer = [0; 4096]; // UDP has max packet size
        let response = loop {
            match self.socket.recv_from(&mut buffer) {
                Ok((size, _)) => {
                    let response: Response = from_bytes(&buffer[..size])
                        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                    if response.request_id == self.request_counter {
                        break response;
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(10));
                    continue;
                }
                Err(e) => return Err(e),
            }
        };

        Ok(response)
    }

    pub fn enable_rc(&mut self) -> Result<(), Box<dyn Error>> {
        self.send_request(RequestType::EnableRC, None)?;
        Ok(())
    }

    pub fn disable_rc(&mut self) -> Result<(), Box<dyn Error>> {
        self.send_request(RequestType::DisableRC, None)?;
        Ok(())
    }

    pub fn reset_aircraft(&mut self) -> Result<(), Box<dyn Error>> {
        self.send_request(RequestType::ResetAircraft, None)?;
        Ok(())
    }

    pub fn exchange_data(
        &mut self,
        control: &ControlInputs,
    ) -> Result<SimulatorState, Box<dyn Error>> {
        let response = self.send_request(RequestType::ExchangeData, Some(control.clone()))?;
        if let Some(state) = response.payload {
            Ok(state)
        } else {
            println!("No payload in response: {:?}", response.status);
            Err("No payload in response".into())
        }
    }
}

pub struct ProxyServer {
    socket: UdpSocket,
}

impl ProxyServer {
    pub fn new(port: u8) -> Result<Self, Box<dyn Error>> {
        let socket = UdpSocket::bind(format!("0.0.0.0:{}", port))?;
        socket.set_nonblocking(true)?;
        Ok(ProxyServer { socket })
    }

    pub fn run(&mut self) -> Result<(), Box<dyn Error>> {
        let config = Configuration {
            simulator_host: "127.0.0.1:18083".to_string(),
            connect_timeout: Duration::from_millis(100),
            ..Default::default()
        };

        let bridge = RealFlightBridge::new(&config).unwrap();
        println!("Server listening on {}", self.socket.local_addr()?);

        let mut buffer = [0; 4096]; // UDP max packet size

        loop {
            match self.socket.recv_from(&mut buffer) {
                Ok((size, client_addr)) => {
                    let request: Request = match from_bytes(&buffer[..size]) {
                        Ok(req) => req,
                        Err(e) => {
                            eprintln!("Failed to deserialize request: {}", e);
                            continue;
                        }
                    };

                    let response = process_request(request, &bridge);
                    let response_bytes = match to_stdvec(&response) {
                        Ok(bytes) => bytes,
                        Err(e) => {
                            eprintln!("Failed to serialize response: {}", e);
                            continue;
                        }
                    };

                    self.socket.send_to(&response_bytes, client_addr)?;
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(10));
                    continue;
                }
                Err(e) => {
                    eprintln!("Error receiving UDP packet: {}", e);
                    continue;
                }
            }
        }
    }
}

fn process_request(request: Request, bridge: &RealFlightBridge) -> Response {
    // Simple mock implementation
    match request.request_type {
        RequestType::EnableRC => {
            if let Err(e) = bridge.enable_rc() {
                println!("Error disabling RC: {}", e);
                Response {
                    request_id: request.request_id,
                    status: ResponseStatus::Error,
                    payload: None,
                }
            } else {
                Response {
                    request_id: request.request_id,
                    status: ResponseStatus::Success,
                    payload: None,
                }
            }
        }
        RequestType::DisableRC => {
            if let Err(e) = bridge.disable_rc() {
                println!("Error disabling RC: {}", e);
                Response {
                    request_id: request.request_id,
                    status: ResponseStatus::Error,
                    payload: None,
                }
            } else {
                Response {
                    request_id: request.request_id,
                    status: ResponseStatus::Success,
                    payload: None,
                }
            }
        }
        RequestType::ResetAircraft => {
            if let Err(e) = bridge.reset_aircraft() {
                println!("Error resetting aircraft: {}", e);
                Response {
                    request_id: request.request_id,
                    status: ResponseStatus::Error,
                    payload: None,
                }
            } else {
                Response {
                    request_id: request.request_id,
                    status: ResponseStatus::Success,
                    payload: None,
                }
            }
        }
        RequestType::ExchangeData => {
            if let Some(payload) = request.payload {
                match bridge.exchange_data(&payload) {
                    Ok(state) => Response {
                        request_id: request.request_id,
                        status: ResponseStatus::Success,
                        payload: Some(state),
                    },
                    Err(e) => {
                        println!("Error exchanging data: {}", e);
                        Response {
                            request_id: request.request_id,
                            status: ResponseStatus::Error,
                            payload: None,
                        }
                    }
                }
            } else {
                Response {
                    request_id: request.request_id,
                    status: ResponseStatus::Error,
                    payload: None,
                }
            }
        }
    }
}
