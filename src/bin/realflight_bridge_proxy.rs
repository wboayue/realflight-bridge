use std::error::Error;

use clap::Parser;
use realflight_bridge::AsyncProxyServer;
use tokio_util::sync::CancellationToken;

/// RealFlight Bridge Proxy server.
///
/// Starts an async proxy server that listens for remote connections and forwards them
/// to the RealFlight simulator. The proxy is expected to run on the same machine as the simulator.
#[derive(Parser)]
#[command(version, about)]
struct Args {
    /// Address to bind the server to
    #[arg(long, default_value = "0.0.0.0:8080")]
    bind_address: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let args = Args::parse();

    let server = AsyncProxyServer::new(&args.bind_address).await?;
    let cancel = CancellationToken::new();

    // Set up Ctrl+C handler for graceful shutdown
    let shutdown_cancel = cancel.clone();
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            println!("\nShutdown signal received, stopping server...");
            shutdown_cancel.cancel();
        }
    });

    server.run(cancel).await?;

    Ok(())
}
