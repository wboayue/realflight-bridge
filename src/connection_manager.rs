use std::{
    net::TcpStream,
    sync::{Arc, Mutex},
    thread,
};

use crossbeam_channel::{bounded, Receiver, Sender, TrySendError};
use log::error;

use crate::StatisticsEngine;

use super::Configuration;

pub(crate) struct ConnectionManager {
    config: Configuration,
    next_socket: Receiver<TcpStream>,
    creator_thread: Option<thread::JoinHandle<()>>,
    running: Arc<Mutex<bool>>,
    statistics: Arc<StatisticsEngine>,
}

impl ConnectionManager {
    pub fn new(
        config: Configuration,
        statistics: Arc<StatisticsEngine>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let (sender, receiver) = bounded(config.buffer_size);

        let running = Arc::new(Mutex::new(true));

        let mut manager = ConnectionManager {
            config,
            next_socket: receiver,
            creator_thread: None,
            running,
            statistics,
        };

        manager.start_socket_creator(sender)?;

        Ok(manager)
    }

    // Start the background thread that creates new connections
    fn start_socket_creator(
        &mut self,
        sender: Sender<TcpStream>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let config = self.config.clone();
        let running = Arc::clone(&self.running);
        let statistics = Arc::clone(&self.statistics);

        for _ in 0..config.buffer_size {
            let stream = Self::create_connection(&config, &statistics)?;
            sender.send(stream).unwrap();
        }

        let handle = thread::spawn(move || {
            while *running.lock().unwrap() {
                if sender.is_full() {
                    thread::sleep(config.retry_delay);
                    continue;
                }

                let connection = Self::create_connection(&config, &statistics).unwrap();
                sender.send(connection).unwrap();
            }
        });

        self.creator_thread = Some(handle);
        Ok(())
    }

    // Create a new TCP connection with timeout
    fn create_connection(
        config: &Configuration,
        statistics: &Arc<StatisticsEngine>,
    ) -> Result<TcpStream, Box<dyn std::error::Error>> {
        let addr = config.simulator_url.parse()?;
        for _ in 0..10 {
            match TcpStream::connect_timeout(&addr, config.connect_timeout) {
                Ok(stream) => {
                    return Ok(stream);
                }
                Err(e) => {
                    statistics.increment_error_count();
                    error!("Error creating connection: {}", e);
                }
            }
        }
        Err("Failed to create connection".into())
    }

    // Get a new connection, consuming it
    pub fn get_connection(&self) -> Result<TcpStream, Box<dyn std::error::Error>> {
        Ok(self.next_socket.recv()?)
    }
}

impl Drop for ConnectionManager {
    fn drop(&mut self) {
        // Signal the creator thread to stop
        if let Ok(mut running) = self.running.lock() {
            *running = false;
        }

        // Wait for the creator thread to finish
        if let Some(handle) = self.creator_thread.take() {
            let _ = handle.join();
        }
    }
}
