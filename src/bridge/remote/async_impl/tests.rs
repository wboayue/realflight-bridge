use super::*;
use crate::bridge::AsyncBridge;
use crate::bridge::wire::test_support::MockProxy;
use std::net::TcpListener;

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
// Operation Tests
// ========================================================================

#[tokio::test]
async fn unit_ops_send_matching_requests() {
    let proxy = MockProxy::respond(Response::success());
    let bridge = AsyncRemoteBridge::new(&proxy.addr).await.unwrap();

    bridge.enable_rc().await.unwrap();
    bridge.disable_rc().await.unwrap();
    bridge.reset_aircraft().await.unwrap();
    drop(bridge);

    let types: Vec<_> = proxy
        .requests()
        .into_iter()
        .map(|r| r.request_type)
        .collect();
    assert_eq!(
        types,
        [
            RequestType::EnableRC,
            RequestType::DisableRC,
            RequestType::ResetAircraft
        ]
    );
}

#[tokio::test]
async fn exchange_data_sends_inputs_and_returns_state() {
    let proxy = MockProxy::respond(Response::success_with(SimulatorState::default()));
    let bridge = AsyncRemoteBridge::new(&proxy.addr).await.unwrap();
    let mut control = ControlInputs::default();
    control.channels[2] = 1.0;

    let state = bridge.exchange_data(&control).await.unwrap();
    assert_eq!(state, SimulatorState::default());
    drop(bridge);

    let requests = proxy.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].request_type, RequestType::ExchangeData);
    assert_eq!(requests[0].payload, Some(control));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_calls_receive_own_responses() {
    // Echo channel 0 back in the state so each caller can check its reply
    let proxy = MockProxy::handle(|request| {
        Response::success_with(SimulatorState {
            current_physics_time_s: request.payload.as_ref().unwrap().channels[0],
            ..SimulatorState::default()
        })
    });
    let bridge = std::sync::Arc::new(AsyncRemoteBridge::new(&proxy.addr).await.unwrap());

    let tasks: Vec<_> = (0..200)
        .map(|i| {
            let bridge = bridge.clone();
            tokio::spawn(async move {
                let mut control = ControlInputs::default();
                control.channels[0] = i as f32;
                let state = bridge.exchange_data(&control).await.unwrap();
                assert_eq!(state.current_physics_time_s, i as f32, "task {i}");
            })
        })
        .collect();

    for task in tasks {
        task.await.unwrap();
    }
}

#[tokio::test]
async fn cancelled_call_does_not_leak_response_to_next_call() {
    // First reply is slow so the caller times out after the request is sent
    let mut first = true;
    let proxy = MockProxy::handle(move |request| {
        if std::mem::take(&mut first) {
            std::thread::sleep(Duration::from_millis(200));
        }
        Response::success_with(SimulatorState {
            current_physics_time_s: request.payload.as_ref().unwrap().channels[0],
            ..SimulatorState::default()
        })
    });
    let bridge = AsyncRemoteBridge::new(&proxy.addr).await.unwrap();

    let mut stale = ControlInputs::default();
    stale.channels[0] = 1.0;
    let cancelled = timeout(Duration::from_millis(50), bridge.exchange_data(&stale)).await;
    assert!(cancelled.is_err(), "first call should time out");

    let mut fresh = ControlInputs::default();
    fresh.channels[0] = 2.0;
    match bridge.exchange_data(&fresh).await {
        Err(BridgeError::Connection(_)) => {}
        Ok(state) => panic!("received stale response: {}", state.current_physics_time_s),
        Err(other) => panic!("expected Connection error, got {:?}", other),
    }
}

// ========================================================================
// Error Handling Tests
// ========================================================================

#[tokio::test]
async fn malformed_response_is_invalid_data() {
    let proxy = MockProxy::reply_raw(vec![0xFF, 0xFF, 0xFF, 0xFF]);
    let bridge = AsyncRemoteBridge::new(&proxy.addr).await.unwrap();

    match bridge.enable_rc().await {
        Err(BridgeError::Connection(e)) => {
            assert_eq!(e.kind(), std::io::ErrorKind::InvalidData);
        }
        other => panic!("expected Connection(InvalidData), got {:?}", other),
    }
}

#[tokio::test]
async fn unit_ops_fail_on_proxy_error() {
    let proxy = MockProxy::respond(Response::error());
    let bridge = AsyncRemoteBridge::new(&proxy.addr).await.unwrap();

    assert!(bridge.enable_rc().await.is_err());
    assert!(bridge.disable_rc().await.is_err());
    assert!(bridge.reset_aircraft().await.is_err());
}

#[tokio::test]
async fn server_disconnect_returns_error() {
    let proxy = MockProxy::hang_up();
    let bridge = AsyncRemoteBridge::new(&proxy.addr).await.unwrap();
    proxy.requests();

    assert!(bridge.enable_rc().await.is_err());
}
