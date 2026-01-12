//! Request handling for the proxy server.

use log::{error, info};
use postcard::{from_bytes, to_stdvec};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::net::TcpStream;
use tokio_util::sync::CancellationToken;

use crate::BridgeError;
use crate::bridge::AsyncBridge;
use crate::bridge::remote::{Request, RequestType, Response};

/// Handles a single client connection.
pub(super) async fn handle_client<B: AsyncBridge>(
    stream: TcpStream,
    bridge: &B,
    cancel: CancellationToken,
) -> Result<(), BridgeError> {
    stream.set_nodelay(true)?;

    let (read_half, write_half) = stream.into_split();
    let mut reader = BufReader::new(read_half);
    let mut writer = BufWriter::new(write_half);
    let mut length_buffer = [0u8; 4];

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                break;
            }
            result = reader.read_exact(&mut length_buffer) => {
                if result.is_err() {
                    break; // Client disconnected
                }

                let msg_length = u32::from_be_bytes(length_buffer) as usize;

                // Read the request data
                let mut buffer = vec![0u8; msg_length];
                reader.read_exact(&mut buffer).await?;

                // Deserialize the request
                let request: Request = match from_bytes(&buffer) {
                    Ok(req) => req,
                    Err(e) => {
                        error!("Failed to deserialize request: {}", e);
                        continue;
                    }
                };

                // Process request
                let response = process_request(request, bridge).await;
                send_response(&mut writer, response).await?;
            }
        }
    }

    info!("Client disconnected");
    Ok(())
}

/// Sends a response to the client.
async fn send_response(
    writer: &mut BufWriter<tokio::net::tcp::OwnedWriteHalf>,
    response: Response,
) -> Result<(), BridgeError> {
    let response_bytes = to_stdvec(&response)
        .map_err(|e| BridgeError::SoapFault(format!("Failed to serialize response: {}", e)))?;
    let length_bytes = (response_bytes.len() as u32).to_be_bytes();

    writer.write_all(&length_bytes).await?;
    writer.write_all(&response_bytes).await?;
    writer.flush().await?;

    Ok(())
}

/// Processes a request using the async bridge.
async fn process_request<B: AsyncBridge>(request: Request, bridge: &B) -> Response {
    match request.request_type {
        RequestType::EnableRC => match bridge.enable_rc().await {
            Ok(()) => Response::success(),
            Err(e) => {
                error!("Error enabling RC: {}", e);
                Response::error()
            }
        },
        RequestType::DisableRC => match bridge.disable_rc().await {
            Ok(()) => Response::success(),
            Err(e) => {
                error!("Error disabling RC: {}", e);
                Response::error()
            }
        },
        RequestType::ResetAircraft => match bridge.reset_aircraft().await {
            Ok(()) => Response::success(),
            Err(e) => {
                error!("Error resetting aircraft: {}", e);
                Response::error()
            }
        },
        RequestType::ExchangeData => match request.payload {
            Some(payload) => match bridge.exchange_data(&payload).await {
                Ok(state) => Response::success_with(state),
                Err(e) => {
                    error!("Error exchanging data: {}", e);
                    Response::error()
                }
            },
            None => Response::error(),
        },
    }
}
