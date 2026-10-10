use std::error::Error;
use std::time::Duration;

use clap::Parser;
use realflight_bridge::AsyncProxyServer;
use tokio_util::sync::CancellationToken;

/// RealFlight Bridge Proxy server.
///
/// Starts an async proxy server that listens for remote connections and forwards them
/// to the RealFlight simulator. The proxy is expected to run on the same machine as the simulator.
/// It serves one client at a time; any new connection replaces the current client.
#[derive(Parser)]
#[command(version, about)]
struct Args {
    /// Address to bind the server to
    #[arg(long, default_value = "0.0.0.0:8080")]
    bind_address: String,

    /// Pre-open the next simulator connection to hide connect latency.
    /// Newer RealFlight versions stall on idle connections; only use with
    /// versions that tolerate them.
    #[arg(long)]
    preconnect: bool,

    /// Milliseconds allowed for each simulator request before the client gets
    /// an error. Keep below the client's I/O timeout (default 100 ms).
    #[arg(long, default_value_t = 50, value_parser = clap::value_parser!(u64).range(1..))]
    request_timeout_ms: u64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let args = Args::parse();

    let server = AsyncProxyServer::new(&args.bind_address)
        .await?
        .preconnect(args.preconnect)
        .request_timeout(Duration::from_millis(args.request_timeout_ms));
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
