use std::sync::Arc;
use std::time::Duration;

use super::*;
use crate::StatisticsEngine;
use crate::soap_client::test_support::Server;

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
    let client = AsyncTcpSoapClient::new(pool);

    let response = client.send_action("ResetAircraft", "").await.unwrap();

    assert_eq!(response.status_code, 200);
    assert!(response.body.contains("ResetAircraftResponse"));
    assert!(server.requests()[0].contains("<ResetAircraft>"));
}
