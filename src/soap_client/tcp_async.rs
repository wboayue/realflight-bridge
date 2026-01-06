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

    fn create_mock_response(body: &str) -> String {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        )
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
}
