use std::sync::Arc;
use std::time::Duration;

use super::*;
use crate::StatisticsEngine;
use crate::soap_client::test_support::{Server, SilentServer};

#[tokio::test]
async fn sends_request_and_receives_response() {
    let server = Server::new(&["reset-aircraft-200"]);
    let addr = format!("127.0.0.1:{}", server.port()).parse().unwrap();
    let pool = AsyncConnectionPool::new(
        addr,
        Duration::from_secs(5),
        1,
        Arc::new(StatisticsEngine::new()),
    )
    .await
    .unwrap();
    pool.ensure_initialized(Duration::from_secs(5))
        .await
        .unwrap();
    let client = AsyncTcpSoapClient::new(pool, Duration::from_secs(5));

    let response = client.send_action("ResetAircraft", "").await.unwrap();

    assert_eq!(response.status_code, 200);
    assert!(response.body.contains("ResetAircraftResponse"));
    assert!(server.requests()[0].contains("<ResetAircraft>"));
}

#[tokio::test]
async fn times_out_when_simulator_never_replies() {
    let server = SilentServer::new();
    let pool = AsyncConnectionPool::new(
        server.addr(),
        Duration::from_secs(1),
        0,
        Arc::new(StatisticsEngine::new()),
    )
    .await
    .unwrap();
    let client = AsyncTcpSoapClient::new(pool, Duration::from_millis(50));

    let start = std::time::Instant::now();
    match client.send_action("ResetAircraft", "").await {
        Err(BridgeError::Connection(e)) => assert_eq!(e.kind(), std::io::ErrorKind::TimedOut),
        other => panic!(
            "expected Connection(TimedOut), got {:?}",
            other.map(|r| r.status_code)
        ),
    }
    assert!(start.elapsed() < Duration::from_secs(1));
}
