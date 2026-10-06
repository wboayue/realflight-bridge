//! Request handling for the proxy server.

use log::{error, info};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::net::TcpStream;
use tokio_util::sync::CancellationToken;

use crate::BridgeError;
use crate::bridge::AsyncBridge;
use crate::bridge::remote::frame::{FRAME_HEADER_LEN, decode_frame, encode_frame, frame_len};
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
    let mut header = [0u8; FRAME_HEADER_LEN];
    let mut buffer = Vec::new();

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                break;
            }
            result = reader.read_exact(&mut header) => {
                if result.is_err() {
                    break; // Client disconnected
                }

                // Read the request data into reusable buffer
                buffer.clear();
                buffer.resize(frame_len(header), 0);
                reader.read_exact(&mut buffer).await?;

                let request: Request = match decode_frame(&buffer) {
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
    writer.write_all(&encode_frame(&response)?).await?;
    writer.flush().await?;

    Ok(())
}

/// Processes a request using the async bridge.
async fn process_request<B: AsyncBridge>(request: Request, bridge: &B) -> Response {
    let result = match &request.request_type {
        RequestType::EnableRC => bridge.enable_rc().await.map(|()| None),
        RequestType::DisableRC => bridge.disable_rc().await.map(|()| None),
        RequestType::ResetAircraft => bridge.reset_aircraft().await.map(|()| None),
        RequestType::ExchangeData => match &request.payload {
            Some(control) => bridge.exchange_data(control).await.map(Some),
            None => Err(BridgeError::SoapFault("Missing control inputs".into())),
        },
    };

    match result {
        Ok(Some(state)) => Response::success_with(state),
        Ok(None) => Response::success(),
        Err(e) => {
            error!("{:?} failed: {}", request.request_type, e);
            Response::error()
        }
    }
}
