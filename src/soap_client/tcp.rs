//! Provides and implementation of a SOAP client that uses the TCP protocol.

use std::{
    error::Error,
    io::{BufRead, BufReader, Read, Write},
    net::TcpStream,
    sync::Arc,
};

use crate::{encode_envelope, ConnectionManager, SoapClient, SoapResponse, StatisticsEngine};

const HEADER_LEN: usize = 120;

pub(crate) struct TcpSoapClient {
    pub(crate) statistics: Arc<StatisticsEngine>,
    pub(crate) connection_manager: ConnectionManager,
}

impl SoapClient for TcpSoapClient {
    fn send_action(&self, action: &str, body: &str) -> Result<SoapResponse, Box<dyn Error>> {
        eprintln!("Sending action: {}", action);

        let envelope = encode_envelope(action, body);
        let mut stream = self.connection_manager.get_connection()?;
        self.send_request(&mut stream, action, &envelope);
        self.statistics.increment_request_count();

        match self.read_response(&mut BufReader::new(stream)) {
            Some(response) => Ok(response),
            None => Err("Failed to read response".into()),
        }
    }
}

impl TcpSoapClient {
    fn send_request(&self, stream: &mut TcpStream, action: &str, envelope: &str) {
        let mut request = String::with_capacity(HEADER_LEN + envelope.len() + action.len());

        request.push_str("POST / HTTP/1.1\r\n");
        request.push_str(&format!("Soapaction: '{}'\r\n", action));
        request.push_str(&format!("Content-Length: {}\r\n", envelope.len()));
        request.push_str("Content-Type: text/xml;charset=utf-8\r\n");
        request.push_str("\r\n");
        request.push_str(envelope);

        stream.write_all(request.as_bytes()).unwrap();
    }

    fn read_response(&self, stream: &mut BufReader<TcpStream>) -> Option<SoapResponse> {
        let mut status_line = String::new();

        if let Err(e) = stream.read_line(&mut status_line) {
            eprintln!("Error reading status line: {}", e);
            return None;
        }

        if status_line.is_empty() {
            return None;
        }
        // eprintln!("Status Line: '{}'", status_line.trim());
        let status_code: u32 = status_line
            .split_whitespace()
            .nth(1)
            .unwrap()
            .parse()
            .unwrap();

        // Read headers
        let mut headers = String::new();
        let mut content_length: Option<usize> = None;
        loop {
            let mut line = String::new();
            stream.read_line(&mut line).unwrap();
            if line == "\r\n" {
                break; // End of headers
            }
            if line.to_lowercase().starts_with("content-length:") {
                if let Some(length) = line.split_whitespace().nth(1) {
                    content_length = length.trim().parse().ok();
                }
            }
            headers.push_str(&line);
        }

        // println!("Headers:\n{}", headers);
        // println!("content length:\n{}", content_length.unwrap());

        // Read the body based on Content-Length
        if let Some(length) = content_length {
            // let mut body = String::with_capacity(length);
            // stream.read_to_string(&mut body).unwrap();

            let mut body = vec![0; length];
            stream.read_exact(&mut body).unwrap();
            let body = String::from_utf8_lossy(&body).to_string();
            // println!("Body: {}", r);

            Some(SoapResponse { status_code, body })
        } else {
            None
        }
    }
}
