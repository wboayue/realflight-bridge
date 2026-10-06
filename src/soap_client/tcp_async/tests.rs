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
