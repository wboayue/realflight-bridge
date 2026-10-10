use std::io::{ErrorKind, Read};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::{Duration, Instant};

use super::*;

mod deadline {
    use super::*;

    #[test]
    fn max_timeout_is_unbounded() {
        let deadline = Deadline::after(Duration::MAX);
        assert_eq!(deadline.remaining("peer").unwrap(), None);
    }

    #[test]
    fn reports_time_left() {
        let left = Deadline::after(Duration::from_secs(5))
            .remaining("peer")
            .unwrap()
            .unwrap();
        assert!(left <= Duration::from_secs(5));
        assert!(left > Duration::from_secs(4));
    }

    #[test]
    fn errors_once_passed() {
        let deadline = Deadline::after(Duration::from_millis(1));
        thread::sleep(Duration::from_millis(5));

        let e = deadline.remaining("simulator").unwrap_err();
        assert_eq!(e.kind(), ErrorKind::TimedOut);
        assert!(e.to_string().contains("simulator"), "{e}");
    }
}

mod ensure_nonzero {
    use super::*;

    #[test]
    fn rejects_zero() {
        let e = ensure_nonzero("io timeout", Duration::ZERO).unwrap_err();
        assert_eq!(e.kind(), ErrorKind::InvalidInput);
        assert!(e.to_string().contains("io timeout"));
    }

    #[test]
    fn accepts_non_zero() {
        assert!(ensure_nonzero("io timeout", Duration::from_millis(1)).is_ok());
    }
}

mod deadline_stream {
    use super::*;

    /// Connected pair: (client side, server side).
    fn pair() -> (TcpStream, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (server, _) = listener.accept().unwrap();
        (client, server)
    }

    #[test]
    fn read_times_out_naming_peer() {
        let (client, _server) = pair();
        let mut stream = DeadlineStream::new(client, "proxy");
        stream.set_deadline(Deadline::after(Duration::from_millis(50)));

        let start = Instant::now();
        let e = stream.read(&mut [0u8; 1]).unwrap_err();
        assert_eq!(e.kind(), ErrorKind::TimedOut);
        assert!(e.to_string().contains("no response from proxy"), "{e}");
        assert!(start.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn reads_without_deadline() {
        let (client, mut server) = pair();
        let mut stream = DeadlineStream::new(client, "proxy");
        std::io::Write::write_all(&mut server, b"x").unwrap();

        let mut buf = [0u8; 1];
        assert_eq!(stream.read(&mut buf).unwrap(), 1);
        assert_eq!(&buf, b"x");
    }
}
