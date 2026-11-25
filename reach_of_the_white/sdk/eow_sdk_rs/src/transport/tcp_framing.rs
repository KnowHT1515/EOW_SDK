use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::transport::SdkError;

const MAX_BODY_LENGTH: usize = 20 * 1024 * 1024;
const SENTINEL: [u8; 5] = *b"\\eow/";

pub async fn read_frame(reader: &mut (impl AsyncRead + Unpin)) -> Result<Option<Vec<u8>>, SdkError> {
    let mut prefix: [u8; 4] = [0; 4];
    if reader.read(&mut prefix[..1]).await? == 0 {
        return Ok(None);
    }
    reader.read_exact(&mut prefix[1..]).await?;
    let length: usize = u32::from_be_bytes(prefix) as usize;
    validate_length(length)?;

    let mut body: Vec<u8> = vec![0; length];
    reader.read_exact(&mut body).await?;
    let mut tail: [u8; SENTINEL.len()] = [0; SENTINEL.len()];
    reader.read_exact(&mut tail).await?;
    if tail != SENTINEL {
        return Err(SdkError::Protocol("invalid frame sentinel"));
    }
    Ok(Some(body))
}

pub async fn write_frame(writer: &mut (impl AsyncWrite + Unpin), body: &[u8]) -> Result<(), SdkError> {
    validate_length(body.len())?;
    writer.write_all(&(body.len() as u32).to_be_bytes()).await?;
    writer.write_all(body).await?;
    writer.write_all(&SENTINEL).await?;
    Ok(())
}

pub fn validate_length(length: usize) -> Result<(), SdkError> {
    if length == 0 || length > MAX_BODY_LENGTH {
        return Err(SdkError::Protocol("frame body length must be between 1 byte and 20 MiB"));
    }
    Ok(())
}
