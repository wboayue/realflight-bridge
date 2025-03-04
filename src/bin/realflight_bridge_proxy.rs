use std::net::{TcpListener, TcpStream};


// Proxy server that listens on
// Only one active connection is supported
fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:8080")?;
    println!("Server listening on 127.0.0.1:8080");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                handle_client(stream);
            },
            Err(e) => {
                eprintln!("Failed to accept connection: {}", e);
            }
        }
    }

    Ok(())
}

fn handle_client(mut stream: TcpStream) {
    println!("New client connected: {}", stream.peer_addr().unwrap());
}
