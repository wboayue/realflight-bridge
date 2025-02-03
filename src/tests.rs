use std::{io, net::TcpListener, sync::atomic::AtomicBool, thread};

use clap::builder::Str;

use super::*;

#[derive(Debug, Clone)]
struct MockResponse {
    response: &'static str,
}

struct MockServer {
    tests: Vec<MockResponse>,
    handle: Option<thread::JoinHandle<()>>,
    running: Arc<AtomicBool>,
}

impl Drop for MockServer {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);

        if let Some(handle) = self.handle.take() {
            handle.join().unwrap();
        }
    }
}

impl MockServer {
    fn new(tests: Vec<MockResponse>) -> Self {
        MockServer {
            tests: tests,
            handle: None,
            running: Arc::new(AtomicBool::new(true)),
        }
    }

    fn setup(&mut self) {
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
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
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

#[test]
pub fn test_activate() {
    let mut server = MockServer::new(vec![
            MockResponse {
                response: "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<SOAP-ENV:Envelope xmlns:SOAP-ENV=\"http://schemas.xmlsoap.org/soap/envelope/\" xmlns:SOAP-ENC=\"http://schemas.xmlsoap.org/soap/encoding/\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:xsd=\"http://www.w3.org/2001/XMLSchema\"><SOAP-ENV:Body><ResetAircraftResponse><unused>0</unused></ResetAircraftResponse></SOAP-ENV:Body></SOAP-ENV:Envelope>"
            },
            MockResponse {
                response: "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<SOAP-ENV:Envelope xmlns:SOAP-ENV=\"http://schemas.xmlsoap.org/soap/envelope/\" xmlns:SOAP-ENC=\"http://schemas.xmlsoap.org/soap/encoding/\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:xsd=\"http://www.w3.org/2001/XMLSchema\"><SOAP-ENV:Body><InjectUAVControllerInterfaceResponse><unused>0</unused></InjectUAVControllerInterfaceResponse></SOAP-ENV:Body></SOAP-ENV:Envelope>",
            }
        ]
    );

    server.setup();

    let configuration = Configuration::default();

    let bridge = RealFlightBridge::new(configuration).unwrap();
    bridge.activate().unwrap();

    // assert_eq!(server.requests()[0], "Activate");
    // assert_eq!(server.requests()[1], "InjectUAVControllerInterface");
}

#[test]
pub fn test_encode_control_inputs() {
    let soap_body = "<pControlInputs><m-selectedChannels>4095</m-selectedChannels><m-channelValues-0to1><item>0</item><item>0</item><item>0</item><item>0</item><item>0</item><item>0</item><item>0</item><item>0</item><item>0</item><item>0</item><item>0</item><item>0</item></m-channelValues-0to1></pControlInputs>";
    let control = ControlInputs::default();
    assert_eq!(encode_control_inputs(&control), soap_body);
}

#[test]
pub fn test_encode_envelope() {
    let envelope = "<?xml version='1.0' encoding='UTF-8'?><soap:Envelope xmlns:soap='http://schemas.xmlsoap.org/soap/envelope/' xmlns:xsd='http://www.w3.org/2001/XMLSchema' xmlns:xsi='http://www.w3.org/2001/XMLSchema-instance'><soap:Body><InjectUAVControllerInterface></InjectUAVControllerInterface></soap:Body></soap:Envelope>";
    assert_eq!(
        encode_envelope("InjectUAVControllerInterface", UNUSED),
        envelope
    );
}
