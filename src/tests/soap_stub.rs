use std::{
    io::{ErrorKind, Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
};

pub struct Server {
    port: u16,
    responses: Vec<String>,
    handle: Option<thread::JoinHandle<()>>,
    running: Arc<AtomicBool>,
    requests: Arc<Mutex<Vec<String>>>,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);

        if let Some(handle) = self.handle.take() {
            handle.join().unwrap();
        }
    }
}

impl Server {
    pub fn new(port: u16, responses: Vec<String>) -> Self {
        Server {
            port,
            responses,
            handle: None,
            running: Arc::new(AtomicBool::new(true)),
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn request_count(&self) -> usize {
        let request = self.requests.lock().unwrap();
        request.len()
    }

    pub fn setup(&mut self) {
        let responses = self.responses.clone();
        let running = Arc::clone(&self.running);
        let requests = Arc::clone(&self.requests);
        let port = self.port;

        let handle = thread::spawn(move || {
            let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).unwrap();
            listener.set_nonblocking(true).unwrap();

            eprintln!("Server listening on port {}...", port);

            let mut responses = responses.iter();

            for stream in listener.incoming() {
                match stream {
                    Ok(mut stream) => {
                        let mut buf: String = String::new();
                        let request = stream.read_to_string(&mut buf);

                        let mut requests = requests.lock().unwrap();
                        eprintln!("Adding request: {}", buf);
                        requests.push(buf);

                        if let Some(response) = responses.next() {
                            handle_client(&stream, response);
                        } else {
                            handle_client(&stream, "error");
                        }
                    }
                    Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                        if running.load(Ordering::Relaxed) {
                            thread::sleep(std::time::Duration::from_millis(100));
                            continue;
                        } else {
                            break;
                        }
                    }
                    Err(e) => eprintln!("Connection failed: {}", e),
                }
            }

            let mut requests = requests.lock().unwrap();
            requests.pop();
        });

        self.handle = Some(handle);
    }

    pub fn requests(&self) -> Vec<String> {
        let requests = self.requests.lock().unwrap();
        requests.clone()
    }
}

fn handle_client(mut stream: &TcpStream, response_key: &str) {
    let body = response_key.as_bytes();

    let mut buffer = String::new();

    buffer.push_str("HTTP/1.1 200 OK\r\n");
    buffer.push_str("Server: gSOAP/2.7\r\n");
    buffer.push_str("Content-Type: text/xml; charset=utf-8\r\n");
    buffer.push_str(&format!("Content-Length: {}\r\n", body.len()));
    buffer.push_str("Connection: close\r\n");
    buffer.push_str("\r\n");
    buffer.push_str(response_key);

    stream.write_all(buffer.as_bytes()).unwrap();
    stream.flush().unwrap();
}
