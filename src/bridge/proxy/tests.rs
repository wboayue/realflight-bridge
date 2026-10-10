use super::*;
use crate::ControlInputs;
use crate::bridge::wire::test_support::{recv, send, write_raw_frame};
use crate::bridge::wire::{RemoteError, Request, RequestType, Response, ResponseStatus};
use fixtures::{CountingSimulator, Proxy, connect, request_within, send_request, wait_until};
use std::io::{Read, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

mod fixtures {
    use super::*;

    /// Proxy server running in a background task.
    pub(super) struct Proxy {
        pub(super) addr: SocketAddr,
        cancel: CancellationToken,
        handle: tokio::task::JoinHandle<Result<(), BridgeError>>,
    }

    impl Proxy {
        /// Serves clients with `bridge` on a default server.
        pub(super) async fn start(bridge: impl AsyncBridge + 'static) -> Self {
            let server = AsyncProxyServer::new("127.0.0.1:0").await.unwrap();
            Self::start_with(server, bridge)
        }

        /// Serves clients with `bridge` on `server`.
        pub(super) fn start_with(
            server: AsyncProxyServer,
            bridge: impl AsyncBridge + 'static,
        ) -> Self {
            Self::spawn(server, |server, cancel| async move {
                server.run_with_bridge(&bridge, cancel).await
            })
        }

        /// Forwards clients to the simulator configured on `server`.
        pub(super) fn forwarding(server: AsyncProxyServer) -> Self {
            Self::spawn(
                server,
                |server, cancel| async move { server.run(cancel).await },
            )
        }

        fn spawn<F, Fut>(server: AsyncProxyServer, run: F) -> Self
        where
            F: FnOnce(AsyncProxyServer, CancellationToken) -> Fut,
            Fut: Future<Output = Result<(), BridgeError>> + Send + 'static,
        {
            let addr = server.local_addr();
            let cancel = CancellationToken::new();
            let handle = tokio::spawn(run(server, cancel.clone()));
            Self {
                addr,
                cancel,
                handle,
            }
        }

        pub(super) fn is_running(&self) -> bool {
            !self.handle.is_finished()
        }

        /// Cancels the server and asserts it shut down cleanly.
        pub(super) async fn stop(self) {
            self.cancel.cancel();
            assert!(self.handle.await.unwrap().is_ok());
        }
    }

    /// Connects a blocking client with `timeout` on reads.
    pub(super) async fn connect(addr: SocketAddr, timeout: Duration) -> std::net::TcpStream {
        tokio::task::spawn_blocking(move || {
            let stream = std::net::TcpStream::connect(addr).unwrap();
            stream.set_read_timeout(Some(timeout)).unwrap();
            stream
        })
        .await
        .unwrap()
    }

    /// Sends `request` on a new connection; the response must arrive within `timeout`.
    pub(super) async fn request_within(
        addr: SocketAddr,
        request: Request,
        timeout: Duration,
    ) -> Response {
        let mut stream = connect(addr, timeout).await;
        tokio::task::spawn_blocking(move || {
            send(&mut stream, &request).unwrap();
            recv(&mut stream).expect("no response within timeout")
        })
        .await
        .unwrap()
    }

    /// Sends `request` on a new connection and waits up to 5 s for the response.
    pub(super) async fn send_request(addr: SocketAddr, request: Request) -> Response {
        request_within(addr, request, Duration::from_secs(5)).await
    }

    /// Fake simulator that counts accepted connections and those the peer closed.
    pub(super) struct CountingSimulator {
        pub(super) addr: SocketAddr,
        opened: Arc<AtomicUsize>,
        closed: Arc<AtomicUsize>,
        task: tokio::task::JoinHandle<()>,
    }

    impl CountingSimulator {
        pub(super) async fn start() -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();
            let opened = Arc::new(AtomicUsize::new(0));
            let closed = Arc::new(AtomicUsize::new(0));
            let (task_opened, task_closed) = (opened.clone(), closed.clone());
            let task = tokio::spawn(async move {
                while let Ok((mut stream, _)) = listener.accept().await {
                    task_opened.fetch_add(1, Ordering::SeqCst);
                    let closed = task_closed.clone();
                    tokio::spawn(async move {
                        let mut buf = [0u8; 64];
                        while matches!(
                            tokio::io::AsyncReadExt::read(&mut stream, &mut buf).await,
                            Ok(n) if n > 0
                        ) {}
                        closed.fetch_add(1, Ordering::SeqCst);
                    });
                }
            });
            Self {
                addr,
                opened,
                closed,
                task,
            }
        }

        pub(super) fn opened(&self) -> usize {
            self.opened.load(Ordering::SeqCst)
        }

        pub(super) fn closed(&self) -> usize {
            self.closed.load(Ordering::SeqCst)
        }
    }

    impl Drop for CountingSimulator {
        fn drop(&mut self) {
            self.task.abort();
        }
    }

    /// Polls `cond` until true, panicking after 2s.
    pub(super) async fn wait_until(what: &str, cond: impl Fn() -> bool) {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
        while !cond() {
            assert!(tokio::time::Instant::now() < deadline, "timed out: {what}");
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }
}

// ========================================================================
// Stub Bridge for Testing
// ========================================================================

struct StubBridge {
    enable_rc_count: Arc<AtomicUsize>,
    disable_rc_count: Arc<AtomicUsize>,
    reset_count: Arc<AtomicUsize>,
    exchange_count: Arc<AtomicUsize>,
}

impl StubBridge {
    fn new() -> Self {
        Self {
            enable_rc_count: Arc::new(AtomicUsize::new(0)),
            disable_rc_count: Arc::new(AtomicUsize::new(0)),
            reset_count: Arc::new(AtomicUsize::new(0)),
            exchange_count: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl AsyncBridge for StubBridge {
    async fn exchange_data(
        &self,
        _control: &ControlInputs,
    ) -> Result<crate::SimulatorState, BridgeError> {
        self.exchange_count.fetch_add(1, Ordering::SeqCst);
        Ok(crate::SimulatorState::default())
    }

    async fn enable_rc(&self) -> Result<(), BridgeError> {
        self.enable_rc_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    async fn disable_rc(&self) -> Result<(), BridgeError> {
        self.disable_rc_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    async fn reset_aircraft(&self) -> Result<(), BridgeError> {
        self.reset_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

// ========================================================================
// Server Creation Tests
// ========================================================================

#[tokio::test]
async fn server_binds_to_address() {
    let server = AsyncProxyServer::new("127.0.0.1:0").await;
    assert!(server.is_ok());

    let server = server.unwrap();
    assert_ne!(server.local_addr().port(), 0);
}

#[tokio::test]
async fn server_returns_local_addr() {
    let server = AsyncProxyServer::new("127.0.0.1:0").await.unwrap();
    let addr = server.local_addr();

    assert_eq!(addr.ip().to_string(), "127.0.0.1");
}

#[tokio::test]
async fn server_binds_to_any_interface() {
    let server = AsyncProxyServer::new("0.0.0.0:0").await;
    assert!(server.is_ok());
}

#[tokio::test]
async fn server_fails_on_invalid_address() {
    let result = AsyncProxyServer::new("invalid:address").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn server_fails_on_privileged_port() {
    // Port 1 requires root privileges
    let result = AsyncProxyServer::new("127.0.0.1:1").await;
    assert!(result.is_err());
}

// ========================================================================
// Cancellation Tests
// ========================================================================

#[tokio::test]
async fn shutdown_via_cancellation_token() {
    let proxy = Proxy::start(StubBridge::new()).await;

    // Give server time to start
    tokio::time::sleep(Duration::from_millis(50)).await;

    proxy.stop().await;
}

mod simulator_connections {
    use super::*;

    #[tokio::test]
    async fn preconnect_opens_only_while_client_connected() {
        let sim = CountingSimulator::start().await;
        let server = AsyncProxyServer::new("127.0.0.1:0")
            .await
            .unwrap()
            .simulator_addr(sim.addr)
            .preconnect(true);
        let proxy = Proxy::forwarding(server);

        // No client: simulator untouched
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(sim.opened(), 0);

        // Client connects: pool pre-opens a connection, but only pool_size of them
        let client = tokio::net::TcpStream::connect(proxy.addr).await.unwrap();
        wait_until("pre-opened connection", || sim.opened() >= 1).await;
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(sim.opened(), 1);

        // Client leaves: every simulator connection is closed
        drop(client);
        wait_until("simulator connections closed", || {
            sim.closed() == sim.opened()
        })
        .await;

        proxy.stop().await;
    }

    #[tokio::test]
    async fn answers_error_when_simulator_never_replies() {
        // CountingSimulator accepts and reads but never responds
        let sim = CountingSimulator::start().await;
        let server = AsyncProxyServer::new("127.0.0.1:0")
            .await
            .unwrap()
            .simulator_addr(sim.addr)
            .request_timeout(Duration::from_millis(50));
        let proxy = Proxy::forwarding(server);

        let start = std::time::Instant::now();
        let request = Request {
            request_type: RequestType::EnableRC,
            payload: None,
        };
        let response = send_request(proxy.addr, request).await;

        assert!(
            matches!(
                response.status,
                ResponseStatus::Error(RemoteError::Connection(_))
            ),
            "expected Connection error, got {:?}",
            response.status
        );
        assert!(start.elapsed() < Duration::from_secs(1));

        proxy.stop().await;
    }

    #[tokio::test]
    async fn default_opens_none_without_requests() {
        let sim = CountingSimulator::start().await;
        let server = AsyncProxyServer::new("127.0.0.1:0")
            .await
            .unwrap()
            .simulator_addr(sim.addr);
        let proxy = Proxy::forwarding(server);

        // A connected client that sends nothing opens no simulator connection
        let client = tokio::net::TcpStream::connect(proxy.addr).await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(sim.opened(), 0);

        drop(client);
        proxy.stop().await;
    }
}

// ========================================================================
// Request Routing Tests
// ========================================================================

#[tokio::test]
async fn routes_enable_rc_to_bridge() {
    let bridge = StubBridge::new();
    let enable_count = bridge.enable_rc_count.clone();
    let proxy = Proxy::start(bridge).await;

    let request = Request {
        request_type: RequestType::EnableRC,
        payload: None,
    };
    let response = send_request(proxy.addr, request).await;

    assert!(matches!(response.status, ResponseStatus::Success));
    assert_eq!(enable_count.load(Ordering::SeqCst), 1);

    proxy.stop().await;
}

#[tokio::test]
async fn routes_disable_rc_to_bridge() {
    let bridge = StubBridge::new();
    let disable_count = bridge.disable_rc_count.clone();
    let proxy = Proxy::start(bridge).await;

    let request = Request {
        request_type: RequestType::DisableRC,
        payload: None,
    };
    let response = send_request(proxy.addr, request).await;

    assert!(matches!(response.status, ResponseStatus::Success));
    assert_eq!(disable_count.load(Ordering::SeqCst), 1);

    proxy.stop().await;
}

#[tokio::test]
async fn routes_reset_aircraft_to_bridge() {
    let bridge = StubBridge::new();
    let reset_count = bridge.reset_count.clone();
    let proxy = Proxy::start(bridge).await;

    let request = Request {
        request_type: RequestType::ResetAircraft,
        payload: None,
    };
    let response = send_request(proxy.addr, request).await;

    assert!(matches!(response.status, ResponseStatus::Success));
    assert_eq!(reset_count.load(Ordering::SeqCst), 1);

    proxy.stop().await;
}

#[tokio::test]
async fn routes_exchange_data_to_bridge() {
    let bridge = StubBridge::new();
    let exchange_count = bridge.exchange_count.clone();
    let proxy = Proxy::start(bridge).await;

    let request = Request {
        request_type: RequestType::ExchangeData,
        payload: Some(ControlInputs::default()),
    };
    let response = send_request(proxy.addr, request).await;

    assert!(matches!(response.status, ResponseStatus::Success));
    assert!(response.payload.is_some());
    assert_eq!(exchange_count.load(Ordering::SeqCst), 1);

    proxy.stop().await;
}

#[tokio::test]
async fn exchange_data_without_payload_returns_error() {
    let proxy = Proxy::start(StubBridge::new()).await;

    let request = Request {
        request_type: RequestType::ExchangeData,
        payload: None, // Missing required payload
    };
    let response = send_request(proxy.addr, request).await;

    assert!(matches!(
        response.status,
        ResponseStatus::Error(RemoteError::Protocol(_))
    ));

    proxy.stop().await;
}

#[tokio::test]
async fn handles_client_disconnect() {
    let proxy = Proxy::start(StubBridge::new()).await;

    // Connect and immediately disconnect
    drop(connect(proxy.addr, Duration::from_secs(5)).await);

    // Server should still be running
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(proxy.is_running());

    proxy.stop().await;
}

// ========================================================================
// Error Path Tests
// ========================================================================

/// Bridge that always returns errors for testing error paths.
struct FailingBridge;

impl AsyncBridge for FailingBridge {
    async fn exchange_data(
        &self,
        _control: &ControlInputs,
    ) -> Result<crate::SimulatorState, BridgeError> {
        Err(BridgeError::SoapFault("Exchange data failed".into()))
    }

    async fn enable_rc(&self) -> Result<(), BridgeError> {
        Err(BridgeError::SoapFault("Enable RC failed".into()))
    }

    async fn disable_rc(&self) -> Result<(), BridgeError> {
        Err(BridgeError::SoapFault("Disable RC failed".into()))
    }

    async fn reset_aircraft(&self) -> Result<(), BridgeError> {
        Err(BridgeError::SoapFault("Reset failed".into()))
    }
}

#[tokio::test]
async fn enable_rc_error_returns_error_response() {
    let proxy = Proxy::start(FailingBridge).await;

    let request = Request {
        request_type: RequestType::EnableRC,
        payload: None,
    };
    let response = send_request(proxy.addr, request).await;

    assert_eq!(
        response.status,
        ResponseStatus::Error(RemoteError::SoapFault("Enable RC failed".into()))
    );

    proxy.stop().await;
}

#[tokio::test]
async fn disable_rc_error_returns_error_response() {
    let proxy = Proxy::start(FailingBridge).await;

    let request = Request {
        request_type: RequestType::DisableRC,
        payload: None,
    };
    let response = send_request(proxy.addr, request).await;

    assert_eq!(
        response.status,
        ResponseStatus::Error(RemoteError::SoapFault("Disable RC failed".into()))
    );

    proxy.stop().await;
}

#[tokio::test]
async fn reset_aircraft_error_returns_error_response() {
    let proxy = Proxy::start(FailingBridge).await;

    let request = Request {
        request_type: RequestType::ResetAircraft,
        payload: None,
    };
    let response = send_request(proxy.addr, request).await;

    assert_eq!(
        response.status,
        ResponseStatus::Error(RemoteError::SoapFault("Reset failed".into()))
    );

    proxy.stop().await;
}

#[tokio::test]
async fn exchange_data_error_returns_error_response() {
    let proxy = Proxy::start(FailingBridge).await;

    let request = Request {
        request_type: RequestType::ExchangeData,
        payload: Some(ControlInputs::default()),
    };
    let response = send_request(proxy.addr, request).await;

    assert_eq!(
        response.status,
        ResponseStatus::Error(RemoteError::SoapFault("Exchange data failed".into()))
    );

    proxy.stop().await;
}

#[tokio::test]
async fn malformed_request_continues_handling() {
    let bridge = StubBridge::new();
    let enable_count = bridge.enable_rc_count.clone();
    let proxy = Proxy::start(bridge).await;

    // Send malformed data, then a valid request
    let mut stream = connect(proxy.addr, Duration::from_secs(5)).await;
    tokio::task::spawn_blocking(move || {
        // Send malformed data
        write_raw_frame(&mut stream, &[0xFF, 0xFF, 0xFF, 0xFF]).unwrap();

        // Now send a valid request
        let request = Request {
            request_type: RequestType::EnableRC,
            payload: None,
        };
        send(&mut stream, &request).unwrap();
        recv::<Response>(&mut stream).unwrap()
    })
    .await
    .unwrap();

    // The valid request should have been processed
    assert_eq!(enable_count.load(Ordering::SeqCst), 1);

    proxy.stop().await;
}

#[tokio::test]
async fn oversized_request_drops_client() {
    let bridge = StubBridge::new();
    let enable_count = bridge.enable_rc_count.clone();
    let proxy = Proxy::start(bridge).await;

    // Announce a ~4 GiB frame; server must close the connection without reading it
    let mut stream = connect(proxy.addr, Duration::from_secs(5)).await;
    let closed = tokio::task::spawn_blocking(move || {
        stream.write_all(&u32::MAX.to_be_bytes()).unwrap();
        stream.flush().unwrap();
        matches!(stream.read(&mut [0u8; 1]), Ok(0))
    })
    .await
    .unwrap();
    assert!(closed, "server should close the connection");

    // Server still serves new clients
    let response = send_request(
        proxy.addr,
        Request {
            request_type: RequestType::EnableRC,
            payload: None,
        },
    )
    .await;
    assert_eq!(response.status, ResponseStatus::Success);
    assert_eq!(enable_count.load(Ordering::SeqCst), 1);

    proxy.stop().await;
}

/// Bridge whose faults are too large to fit in a frame.
struct OversizedFaultBridge;

fn oversized_fault() -> BridgeError {
    BridgeError::SoapFault("x".repeat(crate::bridge::wire::frame::MAX_FRAME_LEN + 1))
}

impl AsyncBridge for OversizedFaultBridge {
    async fn exchange_data(
        &self,
        _control: &ControlInputs,
    ) -> Result<crate::SimulatorState, BridgeError> {
        Err(oversized_fault())
    }

    async fn enable_rc(&self) -> Result<(), BridgeError> {
        Err(oversized_fault())
    }

    async fn disable_rc(&self) -> Result<(), BridgeError> {
        Err(oversized_fault())
    }

    async fn reset_aircraft(&self) -> Result<(), BridgeError> {
        Err(oversized_fault())
    }
}

#[tokio::test]
async fn oversized_response_returns_protocol_error() {
    let proxy = Proxy::start(OversizedFaultBridge).await;

    let request = Request {
        request_type: RequestType::EnableRC,
        payload: None,
    };
    let response = send_request(proxy.addr, request).await;

    match response.status {
        ResponseStatus::Error(RemoteError::Protocol(msg)) => {
            assert!(msg.contains("exceeds limit"), "unexpected message: {msg}")
        }
        other => panic!("expected Protocol error, got {:?}", other),
    }

    proxy.stop().await;
}

// ========================================================================
// Client Replacement Tests
// ========================================================================

mod client_replacement {
    use super::*;

    fn enable_rc() -> Request {
        Request {
            request_type: RequestType::EnableRC,
            payload: None,
        }
    }

    /// Bridge whose first `enable_rc` takes `delay`; later calls answer at once.
    struct SlowFirstBridge {
        delay: Duration,
        calls: AtomicUsize,
    }

    impl SlowFirstBridge {
        fn new(delay: Duration) -> Self {
            Self {
                delay,
                calls: AtomicUsize::new(0),
            }
        }
    }

    impl AsyncBridge for SlowFirstBridge {
        async fn exchange_data(
            &self,
            _control: &ControlInputs,
        ) -> Result<crate::SimulatorState, BridgeError> {
            Ok(crate::SimulatorState::default())
        }

        async fn enable_rc(&self) -> Result<(), BridgeError> {
            if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
                tokio::time::sleep(self.delay).await;
            }
            Ok(())
        }

        async fn disable_rc(&self) -> Result<(), BridgeError> {
            Ok(())
        }

        async fn reset_aircraft(&self) -> Result<(), BridgeError> {
            Ok(())
        }
    }

    /// Sends `enable_rc` on a client that gives up after `timeout` but keeps
    /// its socket open.
    async fn timed_out_client(addr: SocketAddr, timeout: Duration) -> std::net::TcpStream {
        let mut stale = connect(addr, timeout).await;
        tokio::task::spawn_blocking(move || {
            send(&mut stale, &enable_rc()).unwrap();
            assert!(recv::<Response>(&mut stale).is_err());
            stale
        })
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn new_client_answered_while_stale_client_open() {
        let proxy = Proxy::start(StubBridge::new()).await;

        // Stale client stays connected and idle
        let _stale = connect(proxy.addr, Duration::from_secs(5)).await;

        let response = request_within(proxy.addr, enable_rc(), Duration::from_millis(100)).await;
        assert_eq!(response.status, ResponseStatus::Success);

        proxy.stop().await;
    }

    #[tokio::test]
    async fn new_client_answered_after_stale_client_timed_out() {
        let proxy = Proxy::start(SlowFirstBridge::new(Duration::from_millis(50))).await;

        let _stale = timed_out_client(proxy.addr, Duration::from_millis(10)).await;

        // Waits at most for the stale client's in-flight request to finish
        let response = request_within(proxy.addr, enable_rc(), Duration::from_millis(200)).await;
        assert_eq!(response.status, ResponseStatus::Success);

        proxy.stop().await;
    }

    #[tokio::test]
    async fn stuck_client_dropped_after_request_timeout() {
        let server = AsyncProxyServer::new("127.0.0.1:0")
            .await
            .unwrap()
            .request_timeout(Duration::from_millis(50));
        // Stale client's request never finishes
        let proxy = Proxy::start_with(server, SlowFirstBridge::new(Duration::from_secs(3600)));

        let _stale = timed_out_client(proxy.addr, Duration::from_millis(10)).await;

        let response = request_within(proxy.addr, enable_rc(), Duration::from_millis(200)).await;
        assert_eq!(response.status, ResponseStatus::Success);

        proxy.stop().await;
    }

    #[tokio::test]
    async fn stale_client_disconnected_on_replace() {
        let proxy = Proxy::start(StubBridge::new()).await;

        let mut stale = connect(proxy.addr, Duration::from_secs(5)).await;
        send_request(proxy.addr, enable_rc()).await;

        // Proxy closed the stale connection: read sees EOF
        let n = tokio::task::spawn_blocking(move || stale.read(&mut [0u8; 1]).unwrap())
            .await
            .unwrap();
        assert_eq!(n, 0);

        proxy.stop().await;
    }

    #[tokio::test]
    async fn replaced_client_releases_simulator_connections() {
        let sim = CountingSimulator::start().await;
        let server = AsyncProxyServer::new("127.0.0.1:0")
            .await
            .unwrap()
            .simulator_addr(sim.addr)
            .preconnect(true);
        let proxy = Proxy::forwarding(server);

        let _stale = tokio::net::TcpStream::connect(proxy.addr).await.unwrap();
        wait_until("stale client's pre-opened connection", || sim.opened() >= 1).await;

        let _client = tokio::net::TcpStream::connect(proxy.addr).await.unwrap();
        wait_until("stale connection closed, new one opened", || {
            sim.closed() >= 1 && sim.opened() >= 2
        })
        .await;

        // Only the new client's connection remains open
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(sim.opened() - sim.closed(), 1);

        proxy.stop().await;
    }
}
