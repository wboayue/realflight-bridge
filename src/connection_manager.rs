use std::{net::TcpStream, sync::{Arc, Mutex}, thread, time::Duration};

use crossbeam_channel::{bounded, Receiver, Sender, TrySendError};

#[derive(Clone, Debug)]
pub struct ConnectionConfig {
    pub simulator_url: String,
    pub connect_timeout: Duration,
    pub retry_delay: Duration,
    pub buffer_size: usize,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        ConnectionConfig {
            simulator_url: "127.0.0.1:18083".to_string(),
            connect_timeout: Duration::from_millis(50),
            retry_delay: Duration::from_millis(5),
            buffer_size: 1,
        }
    }
}

pub (crate) struct ConnectionManager {
    config: ConnectionConfig,
    next_socket: Receiver<TcpStream>,
    creator_thread: Option<thread::JoinHandle<()>>,
    running: Arc<Mutex<bool>>,    
}

impl ConnectionManager {
    pub fn new(config: ConnectionConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let (sender, receiver) = bounded(config.buffer_size);

        let running = Arc::new(Mutex::new(true));

        let mut manager = ConnectionManager {
            config,
            next_socket: receiver,
            creator_thread: None,
            running,
        };

        manager.start_socket_creator(sender)?;

        Ok(manager)
    }

    // Start the background thread that creates new connections
    fn start_socket_creator(&mut self, sender: Sender<TcpStream>) -> Result<(), Box<dyn std::error::Error>> {
        let config = self.config.clone();
        let running = Arc::clone(&self.running);

        for _ in 0..config.buffer_size {
            let c = Self::create_connection(&config).unwrap();
            sender.send(c).unwrap();
        }

        let handle = thread::spawn(move || {
            let mut connection = Some(Self::create_connection(&config).unwrap());
            while *running.lock().unwrap() {
                match sender.try_send(connection.take().unwrap()) {
                    Ok(_) => {
                        connection = Some(Self::create_connection(&config).unwrap());
                    }
                    Err(TrySendError::Full(_connection)) => {
                        connection = Some(_connection);
                        thread::sleep(config.retry_delay);
                    }
                    Err(TrySendError::Disconnected(_connection)) => {
                        break;
                    }
                }
            }
        });

        self.creator_thread = Some(handle);
        Ok(())
    }

    // Create a new TCP connection with timeout
    fn create_connection(config: &ConnectionConfig) -> Result<TcpStream, Box<dyn std::error::Error>> {
        let addr = config.simulator_url.parse()?;
        let stream = TcpStream::connect_timeout(&addr, config.connect_timeout)?;
        // stream.set_nonblocking(true)?;
        Ok(stream)
        // Ok(TcpStream::connect(&config.simulator_url)?)
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
