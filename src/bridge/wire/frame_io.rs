//! Frame reading over blocking and async streams.

use std::io::{self, Read};

use super::frame::{FRAME_HEADER_LEN, frame_len};

/// Reads one frame's payload into `buf`, replacing its contents.
pub(crate) fn read_frame<R: Read>(reader: &mut R, buf: &mut Vec<u8>) -> io::Result<()> {
    let mut header = [0u8; FRAME_HEADER_LEN];
    reader.read_exact(&mut header)?;
    buf.clear();
    buf.resize(frame_len(header)?, 0);
    reader.read_exact(buf)
}

/// Async version of [read_frame].
#[cfg(feature = "rt-tokio")]
pub(crate) async fn read_frame_async<R>(reader: &mut R, buf: &mut Vec<u8>) -> io::Result<()>
where
    R: tokio::io::AsyncRead + Unpin,
{
    use tokio::io::AsyncReadExt;

    let mut header = [0u8; FRAME_HEADER_LEN];
    reader.read_exact(&mut header).await?;
    buf.clear();
    buf.resize(frame_len(header)?, 0);
    reader.read_exact(buf).await?;
    Ok(())
}
