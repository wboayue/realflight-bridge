use std::{
    io::{ErrorKind, Read, Write}, net::{TcpListener, TcpStream}, sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    }, thread
};
use std::path::PathBuf;
use std::io::BufReader;
use std::io::{BufRead};


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

        // eprintln!("sending shutdown command");
        // let mut stream = TcpStream::connect(format!("127.0.0.1:{}", self.port)).unwrap();
        // let buf = "SHUTDOWN\r\n".as_bytes();
        // stream.write_all(buf).unwrap();
        // stream.flush().unwrap();
        // eprintln!("sent shutdown command");

        if let Some(handle) = self.handle.take() {
            if let Err(e) = handle.join() {
                eprintln!("error shutting down server: {:?}", e);
            }
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

            eprintln!("server listening on port {}", port);

            let mut responses = responses.iter();

            for mut incoming in listener.incoming() {
                eprintln!("incoming connection");
                if !running.load(Ordering::Relaxed) {
                    break;
                }

                if let Err(ref e) = incoming {
                    eprintln!("connection error: {}", e);
                    thread::sleep(std::time::Duration::from_millis(100));
                    continue;                    
                } else if let Ok(ref mut stream) = incoming {
                    let mut a = &mut stream.try_clone().unwrap();
                    let mut streamb = BufReader::new(a);
                    let mut line = String::new();
                    if let Err(e) = streamb.read_line(&mut line) {
                        eprintln!("error reading line: {}", e);
                        continue;
                    } else {
                        eprintln!("status line: {}", line);
                    }

                    let request_body = read_request_body(&mut streamb);
                    if request_body.is_empty() {
                        eprintln!("empty request. try next.");
                        thread::sleep(std::time::Duration::from_millis(100));
                        continue;
                    }

                    // eprintln!("recording request:\n{}", request_body);
                    record_request(&requests, &request_body);

                    if let Some(response_key) = responses.next() {
                        send_response(stream, response_key);
                    } else {
                        eprintln!("no more responses to send");
                    }
                }
            }
            
            eprintln!("server shutting down");
        });

        self.handle = Some(handle);
    }

    pub fn requests(&self) -> Vec<String> {
        let requests = self.requests.lock().unwrap();
        requests.clone()
    }
}

fn read_request_body(stream: &mut BufReader<&mut TcpStream>) -> String {
    let content_length = content_length(stream);

    eprintln!("body content length: {}", content_length);
    if content_length == 0 {
        return String::new();
    }

    let mut request_body = vec![0; content_length];
    stream.read_exact(&mut request_body).unwrap();
    
    String::from_utf8_lossy(&request_body).to_string()
}

fn content_length(stream: &mut BufReader<&mut TcpStream>) -> usize{
    let mut content_length: Option<usize> = None;
    loop {
        let mut line = String::new();
        if let Err(e) = stream.read_line(&mut line) {
            return 0;
        }
        
        // eprintln!("line: {}", line);

        if line == "\r\n" {
            break;
        }

        if line.to_lowercase().starts_with("content-length:") {
            if let Some(length) = line.split_whitespace().nth(1) {
                content_length = length.trim().parse().ok();
            }
        }
    }
    content_length.unwrap_or(0)    
}

fn record_request(requests: &Arc<Mutex<Vec<String>>>, request: &str) {
    let mut requests = requests.lock().unwrap();
    requests.push(request.to_string());
}

fn send_response(mut stream: &TcpStream, response_key: &str) {
    let response_path: PathBuf = [env!("CARGO_MANIFEST_DIR"), "testdata", "responses", &format!("{}.xml", response_key)].iter().collect();
    eprintln!("Response path: {:?}", response_path);
    let body = std::fs::read_to_string(response_path).unwrap();

    let mut buffer = String::new();

    buffer.push_str("HTTP/1.1 200 OK\r\n");
    buffer.push_str("Server: gSOAP/2.7\r\n");
    buffer.push_str("Content-Type: text/xml; charset=utf-8\r\n");
    buffer.push_str(&format!("Content-Length: {}\r\n", body.as_bytes().len()));
    buffer.push_str("Connection: close\r\n");
    buffer.push_str("\r\n");
    buffer.push_str(&body);

//    eprintln!("sending response:\n{}", buffer);
    stream.write_all(buffer.as_bytes()).unwrap();
    stream.flush().unwrap();
}
