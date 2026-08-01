use std::fmt;
use std::io::{self, Read, Write};

pub const MAX_FRAME_BODY: usize = 20 * 1024 * 1024;
pub const SENTINEL: [u8; 5] = *b"\\eow/";

#[derive(Debug)]
pub enum FrameError {
    Io(io::Error),
    ZeroLength,
    Oversize { actual: usize, maximum: usize },
    TruncatedPrefix,
    TruncatedBody,
    TruncatedSentinel,
    SentinelMismatch,
}

impl fmt::Display for FrameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "frame I/O failed: {error}"),
            Self::ZeroLength => formatter.write_str("frame length must not be zero"),
            Self::Oversize { actual, maximum } => {
                write!(
                    formatter,
                    "frame body is {actual} bytes; maximum is {maximum}"
                )
            }
            Self::TruncatedPrefix => formatter.write_str("connection ended during frame prefix"),
            Self::TruncatedBody => formatter.write_str("connection ended during frame body"),
            Self::TruncatedSentinel => {
                formatter.write_str("connection ended during frame sentinel")
            }
            Self::SentinelMismatch => formatter.write_str("frame sentinel does not match \\eow/"),
        }
    }
}

impl std::error::Error for FrameError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for FrameError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Read one complete frame. Clean EOF before a new prefix returns `Ok(None)`.
pub fn read_frame<R: Read>(reader: &mut R) -> Result<Option<Vec<u8>>, FrameError> {
    let mut prefix = [0_u8; 4];
    let mut filled = 0;

    while filled < prefix.len() {
        match reader.read(&mut prefix[filled..]) {
            Ok(0) if filled == 0 => return Ok(None),
            Ok(0) => return Err(FrameError::TruncatedPrefix),
            Ok(read) => filled += read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(FrameError::Io(error)),
        }
    }

    let body_len = u32::from_be_bytes(prefix) as usize;
    if body_len == 0 {
        return Err(FrameError::ZeroLength);
    }
    if body_len > MAX_FRAME_BODY {
        return Err(FrameError::Oversize {
            actual: body_len,
            maximum: MAX_FRAME_BODY,
        });
    }

    let mut body = vec![0_u8; body_len];
    read_exact_part(reader, &mut body, FrameError::TruncatedBody)?;

    let mut sentinel = [0_u8; SENTINEL.len()];
    read_exact_part(reader, &mut sentinel, FrameError::TruncatedSentinel)?;
    if sentinel != SENTINEL {
        return Err(FrameError::SentinelMismatch);
    }

    Ok(Some(body))
}

pub fn write_frame<W: Write>(writer: &mut W, body: &[u8]) -> Result<(), FrameError> {
    if body.is_empty() {
        return Err(FrameError::ZeroLength);
    }
    if body.len() > MAX_FRAME_BODY {
        return Err(FrameError::Oversize {
            actual: body.len(),
            maximum: MAX_FRAME_BODY,
        });
    }

    let prefix = (body.len() as u32).to_be_bytes();
    writer.write_all(&prefix)?;
    writer.write_all(body)?;
    writer.write_all(&SENTINEL)?;
    writer.flush()?;
    Ok(())
}

fn read_exact_part<R: Read>(
    reader: &mut R,
    buffer: &mut [u8],
    truncated: FrameError,
) -> Result<(), FrameError> {
    let mut filled = 0;
    while filled < buffer.len() {
        match reader.read(&mut buffer[filled..]) {
            Ok(0) => return Err(truncated),
            Ok(read) => filled += read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(FrameError::Io(error)),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Read};

    use super::*;

    struct ChunkedReader<R> {
        inner: R,
        maximum: usize,
    }

    impl<R: Read> Read for ChunkedReader<R> {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            let limit = buffer.len().min(self.maximum);
            self.inner.read(&mut buffer[..limit])
        }
    }

    #[test]
    fn frame_round_trips_across_fragmented_reads() {
        let body = br#"{"hello":"white"}"#;
        let mut bytes = Vec::new();
        write_frame(&mut bytes, body).unwrap();

        let mut reader = ChunkedReader {
            inner: Cursor::new(bytes),
            maximum: 2,
        };
        assert_eq!(read_frame(&mut reader).unwrap(), Some(body.to_vec()));
        assert_eq!(read_frame(&mut reader).unwrap(), None);
    }

    #[test]
    fn consecutive_frames_remain_aligned() {
        let mut bytes = Vec::new();
        write_frame(&mut bytes, b"first").unwrap();
        write_frame(&mut bytes, b"second").unwrap();

        let mut reader = Cursor::new(bytes);
        assert_eq!(read_frame(&mut reader).unwrap(), Some(b"first".to_vec()));
        assert_eq!(read_frame(&mut reader).unwrap(), Some(b"second".to_vec()));
        assert_eq!(read_frame(&mut reader).unwrap(), None);
    }

    #[test]
    fn invalid_length_and_sentinel_abort_the_stream() {
        let mut zero = Cursor::new([0_u8; 4]);
        assert!(matches!(read_frame(&mut zero), Err(FrameError::ZeroLength)));

        let mut invalid = Vec::new();
        invalid.extend_from_slice(&1_u32.to_be_bytes());
        invalid.push(b'x');
        invalid.extend_from_slice(b"wrong");
        assert!(matches!(
            read_frame(&mut Cursor::new(invalid)),
            Err(FrameError::SentinelMismatch)
        ));
    }

    #[test]
    fn partial_frame_is_reported() {
        let mut prefix = Cursor::new([0_u8, 0]);
        assert!(matches!(
            read_frame(&mut prefix),
            Err(FrameError::TruncatedPrefix)
        ));

        let mut body = Vec::new();
        body.extend_from_slice(&3_u32.to_be_bytes());
        body.extend_from_slice(b"xy");
        assert!(matches!(
            read_frame(&mut Cursor::new(body)),
            Err(FrameError::TruncatedBody)
        ));
    }
}
