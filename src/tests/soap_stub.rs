use std::{io::{ErrorKind, Write}, net::{TcpListener, TcpStream}, sync::{atomic::{AtomicBool, Ordering}, Arc}, thread};

#[derive(Debug, Clone)]
pub struct MockResponse {
    pub response: &'static str,
}

pub struct Server {
    tests: Vec<MockResponse>,
    handle: Option<thread::JoinHandle<()>>,
    running: Arc<AtomicBool>,
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
    pub fn new(tests: Vec<MockResponse>) -> Self {
        Server {
            tests: tests,
            handle: None,
            running: Arc::new(AtomicBool::new(true)),
        }
    }

    pub fn setup(&mut self) {
        let tests = self.tests.clone();
        let running = Arc::clone(&self.running);

        let handle = thread::spawn(move || {
            let listener = TcpListener::bind("0.0.0.0:18083").unwrap();
            listener.set_nonblocking(true).unwrap();

            println!("Server listening on port 18083...");

            for stream in listener.incoming() {
                match stream {
                    Ok(stream) => {
                        handle_client(stream, &tests);
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
        });
        self.handle = Some(handle);
    }

    fn requests(&self) -> Vec<&str> {
        vec![]
    }
}

fn handle_client(mut stream: TcpStream, tests: &Vec<MockResponse>) {
    let body = tests[0].response.as_bytes();

    let mut buffer = String::new();

    buffer.push_str("HTTP/1.1 200 OK\r\n");
    buffer.push_str("Server: gSOAP/2.7\r\n");
    buffer.push_str("Content-Type: text/xml; charset=utf-8\r\n");
    buffer.push_str(&format!("Content-Length: {}\r\n", body.len()));
    buffer.push_str("Connection: close\r\n");
    buffer.push_str("\r\n");
    buffer.push_str(tests[0].response);

    stream.write_all(buffer.as_bytes()).unwrap();
}
