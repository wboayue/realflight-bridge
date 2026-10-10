use std::io::Write;
use std::net::{SocketAddr, TcpListener};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use super::*;
use crate::StatisticsEngine;
use crate::soap_client::test_support::Server;

fn client(addr: SocketAddr, request_timeout: Duration) -> TcpSoapClient {
    let pool = ConnectionPool::new(
        addr,
        Duration::from_secs(1),
        0,
        Arc::new(StatisticsEngine::new()),
    )
    .unwrap();
    TcpSoapClient::new(pool, request_timeout)
}

/// Accepts one connection and runs `serve` on it.
fn listen(serve: impl FnOnce(TcpStream) + Send + 'static) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        serve(stream);
    });
    addr
}

fn assert_timed_out(result: Result<SoapResponse, BridgeError>) {
    match result {
        Err(BridgeError::Connection(e)) => assert_eq!(e.kind(), ErrorKind::TimedOut),
        other => panic!(
            "expected Connection(TimedOut), got {:?}",
            other.map(|r| r.status_code)
        ),
    }
}

#[test]
fn sends_request_and_receives_response() {
    let server = Server::new(&["reset-aircraft-200"]);
    let addr = format!("127.0.0.1:{}", server.port()).parse().unwrap();

    let response = client(addr, Duration::from_secs(5))
        .send_action("ResetAircraft", "")
        .unwrap();

    assert_eq!(response.status_code, 200);
    assert!(response.body.contains("ResetAircraftResponse"));
}

#[test]
fn times_out_when_simulator_never_replies() {
    // Holds the connection open without responding
    let addr = listen(|stream| {
        thread::sleep(Duration::from_secs(2));
        drop(stream);
    });

    let start = Instant::now();
    assert_timed_out(client(addr, Duration::from_millis(50)).send_action("ResetAircraft", ""));
    assert!(start.elapsed() < Duration::from_secs(1));
}

#[test]
fn deadline_covers_whole_response() {
    // Trickles bytes faster than the timeout, so only a whole-request deadline trips
    let addr = listen(|mut stream| {
        for _ in 0..100 {
            if stream.write_all(b"X").is_err() {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
    });

    let start = Instant::now();
    assert_timed_out(client(addr, Duration::from_millis(100)).send_action("ResetAircraft", ""));
    assert!(start.elapsed() < Duration::from_millis(500));
}
