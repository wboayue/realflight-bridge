use super::*;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener as TokioTcpListener;

#[tokio::test]
async fn pool_creates_connections() {
    let listener = TokioTcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let stats = Arc::new(StatisticsEngine::new());

    // Accept connections in background using async listener
    let accept_handle = tokio::spawn(async move {
        // Accept enough for initial pool + some extra
        for _ in 0..5 {
            if tokio::time::timeout(Duration::from_secs(2), listener.accept())
                .await
                .is_err()
            {
                break;
            }
        }
    });

    let pool = AsyncConnectionPool::new(addr, Duration::from_secs(1), 2, stats)
        .await
        .unwrap();

    pool.ensure_initialized(Duration::from_secs(5))
        .await
        .unwrap();

    let conn = pool.get_connection().await;
    assert!(conn.is_ok());

    drop(pool);
    accept_handle.abort();
}

#[tokio::test]
async fn pool_fails_on_invalid_address() {
    let stats = Arc::new(StatisticsEngine::new());

    // Use a port that's unlikely to be listening
    let addr: SocketAddr = "127.0.0.1:1".parse().unwrap();

    let pool = AsyncConnectionPool::new(addr, Duration::from_millis(100), 1, stats)
        .await
        .unwrap();

    // Should timeout waiting for initialization
    let result = pool.ensure_initialized(Duration::from_millis(500)).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn get_connection_returns_valid_stream() {
    let listener = TokioTcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let stats = Arc::new(StatisticsEngine::new());

    let accept_handle = tokio::spawn(async move {
        // Accept and echo back
        if let Ok((mut stream, _)) = listener.accept().await {
            let mut buf = [0u8; 5];
            let _ = tokio::io::AsyncReadExt::read_exact(&mut stream, &mut buf).await;
            let _ = stream.write_all(&buf).await;
        }
    });

    let pool = AsyncConnectionPool::new(addr, Duration::from_secs(1), 1, stats)
        .await
        .unwrap();

    pool.ensure_initialized(Duration::from_secs(5))
        .await
        .unwrap();

    let mut conn = pool.get_connection().await.unwrap();

    // Verify we can write and read from the connection
    conn.write_all(b"hello").await.unwrap();
    let mut buf = [0u8; 5];
    tokio::io::AsyncReadExt::read_exact(&mut conn, &mut buf)
        .await
        .unwrap();
    assert_eq!(&buf, b"hello");

    let _ = accept_handle.await;
}

#[tokio::test]
async fn pool_replenishes_after_use() {
    let listener = TokioTcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let stats = Arc::new(StatisticsEngine::new());

    // Accept multiple connections concurrently
    let accept_handle = tokio::spawn(async move {
        for _ in 0..5 {
            // Use timeout to avoid blocking forever
            let _ = tokio::time::timeout(Duration::from_secs(2), listener.accept()).await;
        }
    });

    // Use pool_size of 2 so replenishment is more visible
    let pool = AsyncConnectionPool::new(addr, Duration::from_millis(500), 2, stats)
        .await
        .unwrap();

    pool.ensure_initialized(Duration::from_secs(5))
        .await
        .unwrap();

    // Get first connection
    let conn1 = pool.get_connection().await;
    assert!(conn1.is_ok());
    drop(conn1);

    // Get second connection (was pre-created)
    let conn2 = pool.get_connection().await;
    assert!(conn2.is_ok());

    drop(pool);
    accept_handle.abort();
}

#[tokio::test]
async fn drop_cancels_background_task() {
    let listener = TokioTcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let stats = Arc::new(StatisticsEngine::new());

    // Accept a few connections
    let accept_handle = tokio::spawn(async move {
        for _ in 0..3 {
            let _ = listener.accept().await;
        }
    });

    let pool = AsyncConnectionPool::new(addr, Duration::from_secs(1), 1, stats)
        .await
        .unwrap();

    pool.ensure_initialized(Duration::from_secs(5))
        .await
        .unwrap();

    // Drop the pool - this should cancel the background task
    drop(pool);

    // Give time for cancellation
    tokio::time::sleep(Duration::from_millis(100)).await;

    // The accept_handle may or may not complete depending on timing
    // but the important thing is no panic/crash
    accept_handle.abort();
}

#[tokio::test]
async fn background_task_increments_error_on_connection_failure() {
    let listener = TokioTcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let stats = Arc::new(StatisticsEngine::new());

    // Accept initial connection then drop listener
    let accept_handle = tokio::spawn(async move {
        let _ = listener.accept().await;
        // Listener dropped here - subsequent connections will fail
    });

    let pool = AsyncConnectionPool::new(addr, Duration::from_millis(50), 1, stats.clone())
        .await
        .unwrap();

    pool.ensure_initialized(Duration::from_secs(5))
        .await
        .unwrap();

    // Consume the initial connection
    let _conn = pool.get_connection().await.unwrap();

    // Wait for accept_handle to finish (listener dropped)
    let _ = accept_handle.await;

    // Wait for background task to try creating connections and fail
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Error count should have increased
    let snapshot = stats.snapshot();
    assert!(
        snapshot.error_count >= 1,
        "expected error_count >= 1, got {}",
        snapshot.error_count
    );
}

#[tokio::test]
async fn init_fails_on_connection_timeout() {
    // Use an address that will timeout (non-routable IP)
    let addr: SocketAddr = "10.255.255.1:18083".parse().unwrap();
    let stats = Arc::new(StatisticsEngine::new());

    let pool = AsyncConnectionPool::new(addr, Duration::from_millis(100), 1, stats)
        .await
        .unwrap();

    // Should fail due to timeout during initialization
    let result = pool.ensure_initialized(Duration::from_millis(500)).await;
    assert!(result.is_err());

    match result {
        Err(BridgeError::Initialization(msg)) => {
            // Either timeout or connection failure message
            assert!(
                msg.contains("timeout")
                    || msg.contains("Connection")
                    || msg.contains("did not initialize"),
                "unexpected error message: {}",
                msg
            );
        }
        other => panic!("expected Initialization error, got {:?}", other),
    }
}

#[tokio::test]
async fn holds_at_most_pool_size_idle_connections() {
    let listener = TokioTcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let stats = Arc::new(StatisticsEngine::new());

    let accepted = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = accepted.clone();
    let accept_handle = tokio::spawn(async move {
        let mut streams = Vec::new();
        while let Ok((stream, _)) = listener.accept().await {
            counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            streams.push(stream);
        }
    });

    let pool = AsyncConnectionPool::new(addr, Duration::from_secs(1), 1, stats)
        .await
        .unwrap();
    pool.ensure_initialized(Duration::from_secs(5))
        .await
        .unwrap();

    // Idle pool must not open a connection beyond pool_size
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(accepted.load(std::sync::atomic::Ordering::SeqCst), 1);

    // Taking a connection frees a slot, so exactly one replacement is opened
    let _conn = pool.get_connection().await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(accepted.load(std::sync::atomic::Ordering::SeqCst), 2);

    drop(pool);
    accept_handle.abort();
}

mod on_demand {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn opens_no_connection_until_requested() {
        let listener = TokioTcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let stats = Arc::new(StatisticsEngine::new());

        let accepted = Arc::new(AtomicUsize::new(0));
        let counter = accepted.clone();
        let accept_handle = tokio::spawn(async move {
            let mut streams = Vec::new();
            while let Ok((stream, _)) = listener.accept().await {
                counter.fetch_add(1, Ordering::SeqCst);
                streams.push(stream);
            }
        });

        let pool = AsyncConnectionPool::new(addr, Duration::from_secs(1), 0, stats)
            .await
            .unwrap();
        pool.ensure_initialized(Duration::from_secs(1))
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(accepted.load(Ordering::SeqCst), 0);

        // Each request opens exactly one connection, none held in reserve
        let _first = pool.get_connection().await.unwrap();
        let _second = pool.get_connection().await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(accepted.load(Ordering::SeqCst), 2);

        accept_handle.abort();
    }

    #[tokio::test]
    async fn returns_error_when_unreachable() {
        let stats = Arc::new(StatisticsEngine::new());
        let addr: SocketAddr = "127.0.0.1:1".parse().unwrap();

        let pool = AsyncConnectionPool::new(addr, Duration::from_millis(100), 0, stats)
            .await
            .unwrap();

        let result = pool.get_connection().await;
        assert!(matches!(result, Err(BridgeError::Connection(_))));
    }
}
