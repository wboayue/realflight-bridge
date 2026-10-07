use super::*;
use std::net::TcpListener;

const INIT_TIMEOUT: Duration = Duration::from_secs(5);

/// Address with no listener; connections are refused.
const UNREACHABLE_ADDR: &str = "127.0.0.1:1";

fn listen() -> (TcpListener, SocketAddr) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    (listener, addr)
}

fn make_pool(
    addr: SocketAddr,
    connect_timeout: Duration,
    pool_size: usize,
    stats: Arc<StatisticsEngine>,
) -> Result<ConnectionPool, BridgeError> {
    ConnectionPool::new(addr, connect_timeout, pool_size, stats)
}

mod pool_creation {
    use super::*;

    #[test]
    fn succeeds_with_listening_server() {
        let (_listener, addr) = listen();

        let stats = Arc::new(StatisticsEngine::new());

        let pool = make_pool(addr, Duration::from_millis(100), 2, stats);
        assert!(pool.is_ok());
    }

    #[test]
    fn fails_when_server_unreachable() {
        let addr = UNREACHABLE_ADDR.parse().unwrap();
        // Don't start a server - connection should fail

        let stats = Arc::new(StatisticsEngine::new());

        let pool = make_pool(addr, Duration::from_millis(100), 2, stats);
        assert!(pool.is_ok());

        let pool = pool.unwrap();
        let result = pool.ensure_initialized(INIT_TIMEOUT);
        assert!(result.is_err());

        match result {
            Err(BridgeError::Initialization(msg)) => {
                assert!(msg.contains("Failed to connect"));
            }
            other => panic!("expected Initialization error, got {:?}", other),
        }
    }
}

mod ensure_initialized {
    use super::*;

    #[test]
    fn returns_ok_when_initialized() {
        let (_listener, addr) = listen();

        let stats = Arc::new(StatisticsEngine::new());

        let pool = make_pool(addr, Duration::from_millis(100), 2, stats).unwrap();
        let result = pool.ensure_initialized(INIT_TIMEOUT);
        assert!(result.is_ok());
    }

    #[test]
    fn multiple_calls_succeed_after_init() {
        let (_listener, addr) = listen();

        let stats = Arc::new(StatisticsEngine::new());

        let pool = make_pool(addr, Duration::from_millis(100), 2, stats).unwrap();

        // First call waits for initialization
        assert!(pool.ensure_initialized(INIT_TIMEOUT).is_ok());
        // Subsequent calls return immediately
        assert!(pool.ensure_initialized(INIT_TIMEOUT).is_ok());
        assert!(pool.ensure_initialized(INIT_TIMEOUT).is_ok());
    }
}

mod get_connection {
    use super::*;
    use std::io::Write;

    #[test]
    fn returns_valid_connection() {
        let (listener, addr) = listen();

        let stats = Arc::new(StatisticsEngine::new());

        let pool = make_pool(addr, Duration::from_millis(100), 2, stats).unwrap();
        pool.ensure_initialized(INIT_TIMEOUT).unwrap();

        // Accept the connections the pool created
        let _conn1 = listener.accept().unwrap();
        let _conn2 = listener.accept().unwrap();

        let conn = pool.get_connection();
        assert!(conn.is_ok());

        // Verify the connection is usable
        let mut stream = conn.unwrap();
        let result = stream.write_all(b"test");
        assert!(result.is_ok());
    }

    #[test]
    fn connections_are_consumed() {
        let (listener, addr) = listen();

        // pool_size = 2, so we can get exactly 2 connections initially
        let stats = Arc::new(StatisticsEngine::new());

        let pool = make_pool(addr, Duration::from_millis(100), 2, stats).unwrap();
        pool.ensure_initialized(INIT_TIMEOUT).unwrap();

        // Accept the connections the pool created
        let _conn1 = listener.accept().unwrap();
        let _conn2 = listener.accept().unwrap();

        // Get both connections from pool
        let conn1 = pool.get_connection();
        assert!(conn1.is_ok());

        let conn2 = pool.get_connection();
        assert!(conn2.is_ok());
    }
}

mod pool_drop {
    use super::*;

    #[test]
    fn stops_creator_thread() {
        let (_listener, addr) = listen();

        let stats = Arc::new(StatisticsEngine::new());

        let pool = make_pool(addr, Duration::from_millis(100), 2, stats).unwrap();
        pool.ensure_initialized(INIT_TIMEOUT).unwrap();

        // Drop should complete without hanging
        drop(pool);
    }

    #[test]
    fn drop_is_safe_before_initialization() {
        let addr = UNREACHABLE_ADDR.parse().unwrap();
        // No listener - pool will fail to initialize

        let stats = Arc::new(StatisticsEngine::new());

        let pool = make_pool(addr, Duration::from_millis(100), 2, stats).unwrap();
        // Don't wait for initialization, just drop
        drop(pool);
    }
}

mod background_connection_creation {
    use super::*;

    #[test]
    fn creates_new_connections_after_consumption() {
        let (listener, addr) = listen();
        listener.set_nonblocking(true).unwrap();

        let stats = Arc::new(StatisticsEngine::new());

        let pool = make_pool(addr, Duration::from_millis(100), 1, stats).unwrap();
        pool.ensure_initialized(INIT_TIMEOUT).unwrap();

        // Accept initial connection
        thread::sleep(Duration::from_millis(50));
        let mut accepted = 0;
        while listener.accept().is_ok() {
            accepted += 1;
        }
        assert!(accepted >= 1, "should have accepted at least 1 connection");

        // Get a connection (consumes it)
        let _conn = pool.get_connection().unwrap();

        // Wait for background thread to create a new one
        thread::sleep(Duration::from_millis(200));

        // Should have created at least one more connection
        let mut more_accepted = 0;
        while listener.accept().is_ok() {
            more_accepted += 1;
        }
        assert!(
            more_accepted >= 1,
            "background should have created more connections"
        );
    }
}

mod error_statistics {
    use super::*;

    #[test]
    fn increments_error_on_connection_failure() {
        let (listener, addr) = listen();

        let stats = Arc::new(StatisticsEngine::new());

        let pool = make_pool(addr, Duration::from_millis(50), 1, stats.clone()).unwrap();
        pool.ensure_initialized(INIT_TIMEOUT).unwrap();

        // Accept initial connection
        let _conn = listener.accept().unwrap();

        // Drop the listener so subsequent connections fail
        drop(listener);

        // Consume the connection to trigger background creation
        let _conn = pool.get_connection().unwrap();

        // Wait for background thread to try creating a connection and fail
        thread::sleep(Duration::from_millis(200));

        // Error count should have increased
        let snapshot = stats.snapshot();
        assert!(
            snapshot.error_count >= 1,
            "expected error_count >= 1, got {}",
            snapshot.error_count
        );
    }
}

mod on_demand {
    use super::*;

    #[test]
    fn opens_no_connection_until_requested() {
        let (listener, addr) = listen();
        listener.set_nonblocking(true).unwrap();

        let pool = make_pool(
            addr,
            Duration::from_secs(1),
            0,
            Arc::new(StatisticsEngine::new()),
        )
        .unwrap();
        pool.ensure_initialized(INIT_TIMEOUT).unwrap();

        thread::sleep(Duration::from_millis(100));
        assert!(listener.accept().is_err(), "no connection expected yet");

        // Each request opens exactly one connection, none held in reserve
        let _first = pool.get_connection().unwrap();
        let _second = pool.get_connection().unwrap();
        thread::sleep(Duration::from_millis(100));
        assert!(listener.accept().is_ok());
        assert!(listener.accept().is_ok());
        assert!(listener.accept().is_err(), "only two connections expected");
    }

    #[test]
    fn returns_error_when_unreachable() {
        let pool = make_pool(
            UNREACHABLE_ADDR.parse().unwrap(),
            Duration::from_millis(100),
            0,
            Arc::new(StatisticsEngine::new()),
        )
        .unwrap();
        pool.ensure_initialized(INIT_TIMEOUT).unwrap();

        let result = pool.get_connection();
        assert!(matches!(result, Err(BridgeError::Connection(_))));
    }
}
