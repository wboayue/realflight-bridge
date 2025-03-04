use std::{
    error::Error,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
};

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
        let request_bytes = rmp_serde::to_vec(&request)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        // Send the length of the request first
        let length_bytes = (request_bytes.len() as u32).to_be_bytes();
        self.stream.write_all(&length_bytes)?;

        // Send the actual request
        self.stream.write_all(&request_bytes)?;
        self.stream.flush()?;

        // Read the response length
        let mut length_buffer = [0u8; 4];
        self.stream.read_exact(&mut length_buffer)?;
        let response_length = u32::from_be_bytes(length_buffer) as usize;

        // Read the response
        let mut response_buffer = vec![0u8; response_length];
        self.stream.read_exact(&mut response_buffer)?;

        // Deserialize the response
        let response: Response = rmp_serde::from_slice(&response_buffer)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

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
            Err("No payload in response".into())
        }
    }
}

pub struct ProxyServer {
    bridge: RealFlightBridge,
}

impl ProxyServer {
    pub fn new(port: u8) -> Self {
        let config = &Configuration::default();
        let bridge = RealFlightBridge::new(config).unwrap();
        ProxyServer { bridge }
    }

    pub fn run(&mut self) -> std::io::Result<()> {
        let listener = TcpListener::bind("127.0.0.1:8080")?;
        println!("Server listening on 127.0.0.1:8080");

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    handle_client(stream);
                }
                Err(e) => {
                    eprintln!("Failed to accept connection: {}", e);
                }
            }
        }

        Ok(())
    }
}

fn handle_client(mut stream: TcpStream) {
    println!("New client connected: {}", stream.peer_addr().unwrap());

    // Buffer to hold the length of the incoming message
    let mut length_buffer = [0u8; 4];

    // Keep handling requests until the client disconnects
    while stream.read_exact(&mut length_buffer).is_ok() {
        // Convert the bytes to a u32 length
        let msg_length = u32::from_be_bytes(length_buffer) as usize;

        // Read the actual message
        let mut buffer = vec![0u8; msg_length];
        if stream.read_exact(&mut buffer).is_err() {
            break;
        }

        // Deserialize the request
        let request: Request = match rmp_serde::from_slice(&buffer) {
            Ok(req) => req,
            Err(e) => {
                eprintln!("Failed to deserialize request: {}", e);
                continue;
            }
        };

        // println!("Received request: {:?}", request);

        // Process the request and create a response
        let response = process_request(request);

        // Serialize the response
        let response_bytes = match rmp_serde::to_vec(&response) {
            Ok(bytes) => bytes,
            Err(e) => {
                eprintln!("Failed to serialize response: {}", e);
                continue;
            }
        };

        // Send the length of the response first
        let length_bytes = (response_bytes.len() as u32).to_be_bytes();
        if stream.write_all(&length_bytes).is_err() {
            break;
        }

        // Send the actual response
        if stream.write_all(&response_bytes).is_err() {
            break;
        }

        // Flush to ensure the response is sent
        if stream.flush().is_err() {
            break;
        }
    }

    println!("Client disconnected: {}", stream.peer_addr().unwrap());
}

fn process_request(request: Request) -> Response {
    // Simple mock implementation
    match request.request_type {
        RequestType::EnableRC => Response {
            request_id: request.request_id,
            status: ResponseStatus::Success,
            payload: None,
        },
        RequestType::DisableRC => Response {
            request_id: request.request_id,
            status: ResponseStatus::Success,
            payload: None,
        },
        RequestType::ResetAircraft => Response {
            request_id: request.request_id,
            status: ResponseStatus::Success,
            payload: None,
        },
        RequestType::ExchangeData => {
            // Mock response with some state
            Response {
                request_id: request.request_id,
                status: ResponseStatus::Success,
                payload: Some(Default::default()),
            }
        }
    }
}
