//! Async implementation of the remote bridge for RealFlight simulator.

use std::net::ToSocketAddrs;
use std::time::Duration;

use log::error;
use postcard::{from_bytes, to_stdvec};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::time::timeout;

use crate::bridge::AsyncBridge;
use crate::{BridgeError, ControlInputs, SimulatorState};

use super::{Request, RequestType, Response};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

/// Builder for AsyncRemoteBridge.
///
/// Configure options synchronously, then call `build()` to connect.
#[derive(Debug, Clone)]
pub struct AsyncRemoteBridgeBuilder {
    address: String,
    connect_timeout: Duration,
}

impl AsyncRemoteBridgeBuilder {
    /// Creates a new builder with the specified address.
    pub fn new(address: &str) -> Self {
        Self {
            address: address.to_string(),
            connect_timeout: DEFAULT_TIMEOUT,
        }
    }

    /// Sets the connection timeout.
    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// Builds the AsyncRemoteBridge, connecting to the server.
    pub async fn build(self) -> Result<AsyncRemoteBridge, BridgeError> {
        let addr = self
            .address
            .to_socket_addrs()
            .map_err(|e| BridgeError::Initialization(format!("Invalid address: {}", e)))?
            .next()
            .ok_or_else(|| BridgeError::Initialization("Invalid address".into()))?;

        let stream = timeout(self.connect_timeout, TcpStream::connect(addr))
            .await
            .map_err(|_| {
                BridgeError::Initialization(format!(
                    "Connection timeout after {:?}",
                    self.connect_timeout
                ))
            })?
            .map_err(|e| BridgeError::Initialization(format!("Connection failed: {}", e)))?;

        stream
            .set_nodelay(true)
            .map_err(|e| BridgeError::Initialization(format!("Failed to set nodelay: {}", e)))?;

        let (read_half, write_half) = stream.into_split();

        Ok(AsyncRemoteBridge {
            reader: Mutex::new(BufReader::new(read_half)),
            writer: Mutex::new(BufWriter::new(write_half)),
            response_buffer: Mutex::new(Vec::with_capacity(4096)),
        })
    }
}

/// Async client for interacting with a remote RealFlight simulator via a proxy server.
///
/// # Examples
///
/// ```no_run
/// use realflight_bridge::{AsyncBridge, AsyncRemoteBridge, ControlInputs};
/// use std::time::Duration;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // Connect to a remote proxy server
///     let bridge = AsyncRemoteBridge::new("192.168.1.100:18083").await?;
///
///     // Or with custom timeout
///     let bridge = AsyncRemoteBridge::builder("192.168.1.100:18083")
///         .timeout(Duration::from_secs(10))
///         .build()
///         .await?;
///
///     // Create sample control inputs
///     let inputs = ControlInputs::default();
///
///     // Exchange data with the simulator
///     let state = bridge.exchange_data(&inputs).await?;
///     println!("Current airspeed: {:?}", state.airspeed);
///
///     Ok(())
/// }
/// ```
pub struct AsyncRemoteBridge {
    reader: Mutex<BufReader<tokio::net::tcp::OwnedReadHalf>>,
    writer: Mutex<BufWriter<tokio::net::tcp::OwnedWriteHalf>>,
    response_buffer: Mutex<Vec<u8>>,
}

impl AsyncBridge for AsyncRemoteBridge {
    async fn exchange_data(&self, control: &ControlInputs) -> Result<SimulatorState, BridgeError> {
        let response = self
            .send_request(RequestType::ExchangeData, Some(control))
            .await?;
        if let Some(state) = response.payload {
            Ok(state)
        } else {
            error!("No payload in response: {:?}", response.status);
            Err(BridgeError::SoapFault("No payload in response".to_string()))
        }
    }

    async fn enable_rc(&self) -> Result<(), BridgeError> {
        self.send_request(RequestType::EnableRC, None).await?;
        Ok(())
    }

    async fn disable_rc(&self) -> Result<(), BridgeError> {
        self.send_request(RequestType::DisableRC, None).await?;
        Ok(())
    }

    async fn reset_aircraft(&self) -> Result<(), BridgeError> {
        self.send_request(RequestType::ResetAircraft, None).await?;
        Ok(())
    }
}

impl AsyncRemoteBridge {
    /// Creates a new AsyncRemoteBridge connected to the specified address.
    pub async fn new(address: &str) -> Result<Self, BridgeError> {
        AsyncRemoteBridgeBuilder::new(address).build().await
    }

    /// Returns a builder for custom configuration.
    pub fn builder(address: &str) -> AsyncRemoteBridgeBuilder {
        AsyncRemoteBridgeBuilder::new(address)
    }

    /// Sends a request to the server and receives a response.
    async fn send_request(
        &self,
        request_type: RequestType,
        payload: Option<&ControlInputs>,
    ) -> Result<Response, BridgeError> {
        let request = Request {
            request_type,
            payload: payload.cloned(),
        };

        // Serialize the request to a byte vector
        let request_bytes = to_stdvec(&request)
            .map_err(|e| BridgeError::SoapFault(format!("Serialization error: {}", e)))?;

        let mut writer = self.writer.lock().await;

        // Send the length of the request (4 bytes)
        let length_bytes = (request_bytes.len() as u32).to_be_bytes();
        writer.write_all(&length_bytes).await?;

        // Send the serialized request data
        writer.write_all(&request_bytes).await?;
        writer.flush().await?;

        drop(writer); // Release lock before reading

        let mut reader = self.reader.lock().await;

        // Read the response length (4 bytes)
        let mut length_buffer = [0u8; 4];
        reader.read_exact(&mut length_buffer).await?;
        let response_length = u32::from_be_bytes(length_buffer) as usize;

        // Read the response data into reusable buffer
        let mut response_buffer = self.response_buffer.lock().await;
        response_buffer.clear();
        response_buffer.resize(response_length, 0);
        reader.read_exact(&mut response_buffer).await?;

        // Deserialize the response
        let response: Response = from_bytes(&response_buffer)
            .map_err(|e| BridgeError::SoapFault(format!("Deserialization error: {}", e)))?;

        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use crate::bridge::AsyncBridge;
    use crate::bridge::remote::Response;

    // ========================================================================
    // Connection Tests
    // ========================================================================

    #[tokio::test]
    async fn connects_to_server() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();

        // Accept one connection in background
        let handle = std::thread::spawn(move || {
            let _ = listener.accept();
        });

        let result = AsyncRemoteBridge::builder(&addr)
            .timeout(Duration::from_secs(1))
            .build()
            .await;

        assert!(result.is_ok());
        let _ = handle.join();
    }

    #[tokio::test]
    async fn builder_sets_timeout() {
        let builder =
            AsyncRemoteBridgeBuilder::new("127.0.0.1:12345").timeout(Duration::from_millis(100));

        assert_eq!(builder.connect_timeout, Duration::from_millis(100));
    }

    #[tokio::test]
    async fn connection_timeout_returns_error() {
        // Use a non-routable address to trigger timeout
        let result = AsyncRemoteBridge::builder("10.255.255.1:12345")
            .timeout(Duration::from_millis(100))
            .build()
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn invalid_address_returns_error() {
        let result = AsyncRemoteBridge::new("not-a-valid-address").await;
        assert!(result.is_err());
    }

    // ========================================================================
    // Helper Functions
    // ========================================================================

    fn mock_server_send_response(mut stream: std::net::TcpStream, response: Response) {
        // Read the request (length + data)
        let mut length_buffer = [0u8; 4];
        stream.read_exact(&mut length_buffer).unwrap();
        let msg_length = u32::from_be_bytes(length_buffer) as usize;
        let mut buffer = vec![0u8; msg_length];
        stream.read_exact(&mut buffer).unwrap();

        // Send response
        let response_bytes = to_stdvec(&response).unwrap();
        let length_bytes = (response_bytes.len() as u32).to_be_bytes();
        stream.write_all(&length_bytes).unwrap();
        stream.write_all(&response_bytes).unwrap();
        stream.flush().unwrap();
    }

    // ========================================================================
    // Operation Tests
    // ========================================================================

    #[tokio::test]
    async fn enable_rc_succeeds() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();

        let handle = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            mock_server_send_response(stream, Response::success());
        });

        let bridge = AsyncRemoteBridge::new(&addr).await.unwrap();
        let result = bridge.enable_rc().await;

        assert!(result.is_ok());
        let _ = handle.join();
    }

    #[tokio::test]
    async fn disable_rc_succeeds() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();

        let handle = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            mock_server_send_response(stream, Response::success());
        });

        let bridge = AsyncRemoteBridge::new(&addr).await.unwrap();
        let result = bridge.disable_rc().await;

        assert!(result.is_ok());
        let _ = handle.join();
    }

    #[tokio::test]
    async fn reset_aircraft_succeeds() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();

        let handle = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            mock_server_send_response(stream, Response::success());
        });

        let bridge = AsyncRemoteBridge::new(&addr).await.unwrap();
        let result = bridge.reset_aircraft().await;

        assert!(result.is_ok());
        let _ = handle.join();
    }

    #[tokio::test]
    async fn exchange_data_succeeds() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();

        let handle = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let state = SimulatorState::default();
            mock_server_send_response(stream, Response::success_with(state));
        });

        let bridge = AsyncRemoteBridge::new(&addr).await.unwrap();
        let control = ControlInputs::default();
        let result = bridge.exchange_data(&control).await;

        assert!(result.is_ok());
        let _ = handle.join();
    }

    // ========================================================================
    // Error Handling Tests
    // ========================================================================

    #[tokio::test]
    async fn exchange_data_no_payload_returns_error() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();

        let handle = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            // Send success but with no payload
            mock_server_send_response(stream, Response::success());
        });

        let bridge = AsyncRemoteBridge::new(&addr).await.unwrap();
        let control = ControlInputs::default();
        let result = bridge.exchange_data(&control).await;

        match result {
            Err(BridgeError::SoapFault(msg)) => {
                assert!(msg.contains("No payload"));
            }
            other => panic!("expected SoapFault, got {:?}", other),
        }
        let _ = handle.join();
    }

    #[tokio::test]
    async fn malformed_response_returns_error() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();

        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            // Read the request
            let mut length_buffer = [0u8; 4];
            stream.read_exact(&mut length_buffer).unwrap();
            let msg_length = u32::from_be_bytes(length_buffer) as usize;
            let mut buffer = vec![0u8; msg_length];
            stream.read_exact(&mut buffer).unwrap();

            // Send malformed response (invalid postcard data)
            let garbage = vec![0xFF, 0xFF, 0xFF, 0xFF];
            let length_bytes = (garbage.len() as u32).to_be_bytes();
            stream.write_all(&length_bytes).unwrap();
            stream.write_all(&garbage).unwrap();
            stream.flush().unwrap();
        });

        let bridge = AsyncRemoteBridge::new(&addr).await.unwrap();
        let result = bridge.enable_rc().await;

        match result {
            Err(BridgeError::SoapFault(msg)) => {
                assert!(msg.contains("Deserialization"));
            }
            other => panic!("expected SoapFault, got {:?}", other),
        }
        let _ = handle.join();
    }

    #[tokio::test]
    async fn server_disconnect_returns_error() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();

        let handle = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            // Close connection immediately without responding
            drop(stream);
        });

        let bridge = AsyncRemoteBridge::new(&addr).await.unwrap();
        let result = bridge.enable_rc().await;

        assert!(result.is_err());
        let _ = handle.join();
    }
}
