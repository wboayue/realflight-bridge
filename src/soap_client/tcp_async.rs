//! Async implementation of a SOAP client that uses the TCP protocol.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

use crate::BridgeError;
use crate::StatisticsEngine;

use super::pool_async::AsyncConnectionPool;
use super::xml::{build_http_request, create_response, parse_content_length, parse_status_line};
use super::{AsyncSoapClient, SoapResponse, encode_envelope};

/// Async implementation of a SOAP client for RealFlight Link that uses the TCP protocol.
pub(crate) struct AsyncTcpSoapClient {
    connection_pool: AsyncConnectionPool,
}

impl AsyncSoapClient for AsyncTcpSoapClient {
    async fn send_action(&self, action: &str, body: &str) -> Result<SoapResponse, BridgeError> {
        let envelope = encode_envelope(action, body);
        let mut stream = self.connection_pool.get_connection().await?;

        // Send request
        let request = build_http_request(action, &envelope);
        stream.write_all(request.as_bytes()).await?;
        stream.flush().await?;

        self.connection_pool.statistics().increment_request_count();

        // Read response
        let mut reader = BufReader::new(stream);

        // Read status line
        let mut status_line = String::new();
        reader.read_line(&mut status_line).await?;
        let status_code = parse_status_line(&status_line)?;

        // Read headers
        let mut content_length: Option<usize> = None;
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).await?;
            if line == "\r\n" {
                break; // End of headers
            }
            if let Some(length) = parse_content_length(&line) {
                content_length = Some(length);
            }
        }

        // Read the body based on Content-Length
        let length = content_length
            .ok_or_else(|| BridgeError::SoapFault("Missing Content-Length header".into()))?;
        let mut body = vec![0; length];
        reader.read_exact(&mut body).await?;

        Ok(create_response(status_code, body))
    }
}

impl AsyncTcpSoapClient {
    /// Creates a new async TCP SOAP client.
    pub async fn new(
        addr: SocketAddr,
        connect_timeout: Duration,
        pool_size: usize,
        statistics: Arc<StatisticsEngine>,
    ) -> Result<Self, BridgeError> {
        let connection_pool =
            AsyncConnectionPool::new(addr, connect_timeout, pool_size, statistics).await?;
        Ok(AsyncTcpSoapClient { connection_pool })
    }

    /// Ensures the connection pool is initialized.
    pub async fn ensure_pool_initialized(&self, init_timeout: Duration) -> Result<(), BridgeError> {
        self.connection_pool.ensure_initialized(init_timeout).await
    }

    /// Returns a reference to the statistics engine.
    #[allow(dead_code)]
    pub fn statistics(&self) -> &Arc<StatisticsEngine> {
        self.connection_pool.statistics()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::net::TcpListener;

    fn create_mock_response_with_status(status_code: u32, body: &str) -> String {
        format!(
            "HTTP/1.1 {} OK\r\nContent-Length: {}\r\n\r\n{}",
            status_code,
            body.len(),
            body
        )
    }

    fn create_mock_response(body: &str) -> String {
        create_mock_response_with_status(200, body)
    }

    #[tokio::test]
    async fn sends_request_and_receives_response() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let stats = Arc::new(StatisticsEngine::new());

        // Spawn a mock server that responds to one request
        let server_handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            // Read request (just consume it)
            let mut buf = [0u8; 1024];
            let _ = std::io::Read::read(&mut stream, &mut buf);

            // Send response
            let response = create_mock_response("<TestResponse>OK</TestResponse>");
            stream.write_all(response.as_bytes()).unwrap();
            stream.flush().unwrap();
        });

        let client = AsyncTcpSoapClient::new(addr, Duration::from_secs(5), 1, stats)
            .await
            .unwrap();

        client
            .ensure_pool_initialized(Duration::from_secs(5))
            .await
            .unwrap();

        let response = client.send_action("TestAction", "").await.unwrap();
        assert_eq!(response.status_code, 200);
        assert!(response.body.contains("TestResponse"));

        server_handle.join().unwrap();
    }

    #[tokio::test]
    async fn parses_200_response_correctly() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let stats = Arc::new(StatisticsEngine::new());

        let server_handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 1024];
            let _ = std::io::Read::read(&mut stream, &mut buf);

            let response = create_mock_response_with_status(200, "<Success/>");
            stream.write_all(response.as_bytes()).unwrap();
            stream.flush().unwrap();
        });

        let client = AsyncTcpSoapClient::new(addr, Duration::from_secs(5), 1, stats)
            .await
            .unwrap();

        client
            .ensure_pool_initialized(Duration::from_secs(5))
            .await
            .unwrap();

        let response = client.send_action("TestAction", "").await.unwrap();
        assert_eq!(response.status_code, 200);

        server_handle.join().unwrap();
    }

    #[tokio::test]
    async fn parses_500_response_correctly() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let stats = Arc::new(StatisticsEngine::new());

        let server_handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 1024];
            let _ = std::io::Read::read(&mut stream, &mut buf);

            let response = create_mock_response_with_status(500, "<Fault>Error</Fault>");
            stream.write_all(response.as_bytes()).unwrap();
            stream.flush().unwrap();
        });

        let client = AsyncTcpSoapClient::new(addr, Duration::from_secs(5), 1, stats)
            .await
            .unwrap();

        client
            .ensure_pool_initialized(Duration::from_secs(5))
            .await
            .unwrap();

        let response = client.send_action("TestAction", "").await.unwrap();
        assert_eq!(response.status_code, 500);
        assert!(response.body.contains("Fault"));

        server_handle.join().unwrap();
    }

    #[tokio::test]
    async fn missing_content_length_returns_error() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let stats = Arc::new(StatisticsEngine::new());

        let server_handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 1024];
            let _ = std::io::Read::read(&mut stream, &mut buf);

            // Send response without Content-Length header
            let response = "HTTP/1.1 200 OK\r\n\r\n<Body/>";
            stream.write_all(response.as_bytes()).unwrap();
            stream.flush().unwrap();
        });

        let client = AsyncTcpSoapClient::new(addr, Duration::from_secs(5), 1, stats)
            .await
            .unwrap();

        client
            .ensure_pool_initialized(Duration::from_secs(5))
            .await
            .unwrap();

        let result = client.send_action("TestAction", "").await;
        match result {
            Err(BridgeError::SoapFault(msg)) => {
                assert!(msg.contains("Content-Length"));
            }
            other => panic!("expected SoapFault, got {:?}", other),
        }

        server_handle.join().unwrap();
    }

    #[tokio::test]
    async fn increments_request_count_on_success() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let stats = Arc::new(StatisticsEngine::new());
        let stats_clone = stats.clone();

        let server_handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 1024];
            let _ = std::io::Read::read(&mut stream, &mut buf);

            let response = create_mock_response("<OK/>");
            stream.write_all(response.as_bytes()).unwrap();
            stream.flush().unwrap();
        });

        let client = AsyncTcpSoapClient::new(addr, Duration::from_secs(5), 1, stats)
            .await
            .unwrap();

        client
            .ensure_pool_initialized(Duration::from_secs(5))
            .await
            .unwrap();

        assert_eq!(stats_clone.snapshot().request_count, 0);

        let _ = client.send_action("TestAction", "").await.unwrap();

        assert_eq!(stats_clone.snapshot().request_count, 1);

        server_handle.join().unwrap();
    }

    #[tokio::test]
    async fn statistics_returns_reference() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let stats = Arc::new(StatisticsEngine::new());
        let stats_clone = stats.clone();

        // Accept connections with timeout
        listener.set_nonblocking(true).unwrap();
        let accept_handle = std::thread::spawn(move || {
            for _ in 0..10 {
                match listener.accept() {
                    Ok(_) => break,
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(100));
                    }
                    Err(_) => break,
                }
            }
        });

        let client = AsyncTcpSoapClient::new(addr, Duration::from_secs(5), 1, stats)
            .await
            .unwrap();

        assert!(Arc::ptr_eq(client.statistics(), &stats_clone));

        drop(client);
        let _ = accept_handle.join();
    }
}
