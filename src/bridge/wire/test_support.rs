//! Blocking test helpers for the remote frame protocol: frame I/O and a mock proxy.

use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread::{self, JoinHandle};

use serde::Serialize;
use serde::de::DeserializeOwned;

use super::frame::{decode_frame, encode_frame};
use super::frame_io::read_frame;
use super::{Request, Response};

/// Writes `payload` as a frame without encoding it (for malformed data).
pub(crate) fn write_raw_frame(stream: &mut impl Write, payload: &[u8]) -> io::Result<()> {
    stream.write_all(&(payload.len() as u32).to_be_bytes())?;
    stream.write_all(payload)?;
    stream.flush()
}

/// Encodes and writes a message as a frame.
#[cfg_attr(not(feature = "rt-tokio"), allow(dead_code))] // used by proxy tests
pub(crate) fn send<T: Serialize>(stream: &mut impl Write, message: &T) -> io::Result<()> {
    stream.write_all(&encode_frame(message).unwrap())?;
    stream.flush()
}

/// Reads one frame and decodes it.
pub(crate) fn recv<T: DeserializeOwned>(stream: &mut impl Read) -> io::Result<T> {
    let mut payload = Vec::new();
    read_frame(stream, &mut payload)?;
    decode_frame(&payload).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// One-connection mock proxy. Answers every request with the same payload
/// until the client disconnects, recording the requests it received.
pub(crate) struct MockProxy {
    pub(crate) addr: String,
    handle: JoinHandle<Vec<Request>>,
}

impl MockProxy {
    /// Replies to each request with `response`.
    pub(crate) fn respond(response: Response) -> Self {
        Self::reply_raw(postcard::to_stdvec(&response).unwrap())
    }

    /// Replies to each request with `payload` framed as-is.
    pub(crate) fn reply_raw(payload: Vec<u8>) -> Self {
        Self::serve(move |_| payload.clone())
    }

    /// Replies to each request with the response built by `handler`.
    #[cfg_attr(not(feature = "rt-tokio"), allow(dead_code))] // used by async tests
    pub(crate) fn handle<F>(mut handler: F) -> Self
    where
        F: FnMut(&Request) -> Response + Send + 'static,
    {
        Self::serve(move |request| postcard::to_stdvec(&handler(request)).unwrap())
    }

    fn serve<F>(mut reply: F) -> Self
    where
        F: FnMut(&Request) -> Vec<u8> + Send + 'static,
    {
        Self::spawn(move |mut stream| {
            let mut requests = Vec::new();
            while let Ok(request) = recv::<Request>(&mut stream) {
                let payload = reply(&request);
                requests.push(request);
                if write_raw_frame(&mut stream, &payload).is_err() {
                    break;
                }
            }
            requests
        })
    }

    /// Replies to the first request with a bare frame header announcing `len`
    /// bytes, then waits for the client to disconnect.
    pub(crate) fn reply_header(len: u32) -> Self {
        Self::spawn(move |mut stream| {
            let mut requests = Vec::new();
            if let Ok(request) = recv::<Request>(&mut stream) {
                requests.push(request);
                let _ = stream.write_all(&len.to_be_bytes());
                let _ = stream.flush();
                let _ = stream.read(&mut [0u8; 1]);
            }
            requests
        })
    }

    /// Accepts the connection and closes it without replying.
    pub(crate) fn hang_up() -> Self {
        Self::spawn(|_stream| Vec::new())
    }

    /// Waits for the client to disconnect and returns the requests received.
    pub(crate) fn requests(self) -> Vec<Request> {
        self.handle.join().unwrap()
    }

    fn spawn<F>(serve: F) -> Self
    where
        F: FnOnce(TcpStream) -> Vec<Request> + Send + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        let handle = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            serve(stream)
        });
        MockProxy { addr, handle }
    }
}
