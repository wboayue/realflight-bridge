//! Provides and implementation of a SOAP client that returns stubbed responses.
//! Useful for testing.

use std::{
    error::Error,
    sync::{Arc, Mutex},
};

use crate::StatisticsEngine;

use super::{SoapClient, SoapResponse, encode_envelope};

#[cfg(test)]
pub(crate) struct StubSoapClient {
    responses: Vec<String>,
    pub(crate) statistics: Option<Arc<StatisticsEngine>>,
    requests: Mutex<Vec<String>>,
}

#[cfg(test)]
impl StubSoapClient {
    pub fn new(responses: Vec<String>) -> Self {
        StubSoapClient {
            responses,
            statistics: None,
            requests: Mutex::new(Vec::new()),
        }
    }

    fn add_request(&self, request: &str) {
        let mut requests = self.requests.lock().unwrap();
        requests.push(request.to_string());
    }

    fn next_response(&self) -> String {
        self.responses.first().unwrap().clone()
    }
}

#[cfg(test)]
impl SoapClient for StubSoapClient {
    fn send_action(&self, action: &str, body: &str) -> Result<SoapResponse, Box<dyn Error>> {
        eprintln!("Sending action: {}", action);

        let envelope = encode_envelope(action, body);

        if let Some(statistics) = &self.statistics {
            statistics.increment_request_count();
        }
        self.add_request(&envelope);

        let response_key = self.next_response();
        let code = response_key.split('-').last().unwrap();

        Ok(SoapResponse {
            status_code: code.parse().unwrap(),
            body: load_response(&response_key),
        })
    }

    fn requests(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }
}

#[cfg(test)]
fn load_response(response_key: &str) -> String {
    use std::path::PathBuf;

    let response_path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "testdata",
        "responses",
        &format!("{}.xml", response_key),
    ]
    .iter()
    .collect();
    eprintln!("Response path: {:?}", response_path);
    let body = std::fs::read_to_string(response_path).unwrap();

    let mut buffer = String::new();

    let code = response_key.split('-').last().unwrap();

    buffer.push_str(&format!("HTTP/1.1 {} OK\r\n", code));
    buffer.push_str("Server: gSOAP/2.7\r\n");
    buffer.push_str("Content-Type: text/xml; charset=utf-8\r\n");
    buffer.push_str(&format!("Content-Length: {}\r\n", body.len()));
    buffer.push_str("Connection: close\r\n");
    buffer.push_str("\r\n");
    buffer.push_str(&body);

    buffer
}
