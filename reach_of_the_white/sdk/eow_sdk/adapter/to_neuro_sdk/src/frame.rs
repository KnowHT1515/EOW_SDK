use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use crate::error::ProtocolError;

pub(crate) const MAX_FRAME_BYTES: usize = 20 * 1024 * 1024;
const SENTINEL: &[u8; 5] = b"\\eow/";

enum ReadStatus {
    Complete,
    Eof,
    Stopped,
}

#[cfg(test)]
pub(crate) fn read_frame(reader: &mut impl Read) -> Result<Option<Vec<u8>>, ProtocolError> {
    read_frame_inner(reader, None)
}

pub(crate) fn read_frame_interruptible(
    reader: &mut impl Read,
    running: &AtomicBool,
) -> Result<Option<Vec<u8>>, ProtocolError> {
    read_frame_inner(reader, Some(running))
}

pub(crate) fn read_frame_until(
    stream: &mut TcpStream,
    deadline: Instant,
) -> Result<Option<Vec<u8>>, ProtocolError> {
    let mut prefix = [0_u8; 4];
    if !read_exact_until(stream, &mut prefix, deadline)? {
        return Ok(None);
    }
    let length = u32::from_be_bytes(prefix) as usize;
    if length == 0 {
        return Err(ProtocolError::Frame("zero-length body"));
    }
    if length > MAX_FRAME_BYTES {
        return Err(ProtocolError::Frame("body exceeds the frame limit"));
    }
    let mut body = vec![0_u8; length];
    if !read_exact_until(stream, &mut body, deadline)? {
        return Err(ProtocolError::Frame("truncated body"));
    }
    let mut sentinel = [0_u8; 5];
    if !read_exact_until(stream, &mut sentinel, deadline)? {
        return Err(ProtocolError::Frame("truncated sentinel"));
    }
    if &sentinel != SENTINEL {
        return Err(ProtocolError::Frame("sentinel mismatch"));
    }
    Ok(Some(body))
}

fn read_exact_until(
    stream: &mut TcpStream,
    buffer: &mut [u8],
    deadline: Instant,
) -> Result<bool, ProtocolError> {
    let mut offset = 0;
    while offset < buffer.len() {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(ProtocolError::Frame("handshake timeout"));
        }
        stream.set_read_timeout(Some(remaining))?;
        match stream.read(&mut buffer[offset..]) {
            Ok(0) if offset == 0 => return Ok(false),
            Ok(0) => return Err(ProtocolError::Frame("truncated frame")),
            Ok(read) => offset += read,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                ) =>
            {
                return Err(ProtocolError::Frame("handshake timeout"));
            }
            Err(error) => return Err(error.into()),
        }
    }
    Ok(true)
}

fn read_frame_inner(
    reader: &mut impl Read,
    running: Option<&AtomicBool>,
) -> Result<Option<Vec<u8>>, ProtocolError> {
    let mut prefix = [0_u8; 4];
    match read_exact(reader, &mut prefix, running)? {
        ReadStatus::Complete => {}
        ReadStatus::Eof | ReadStatus::Stopped => return Ok(None),
    }

    let length = u32::from_be_bytes(prefix) as usize;
    if length == 0 {
        return Err(ProtocolError::Frame("zero-length body"));
    }
    if length > MAX_FRAME_BYTES {
        return Err(ProtocolError::Frame("body exceeds the frame limit"));
    }

    let mut body = vec![0_u8; length];
    if !matches!(
        read_exact(reader, &mut body, running)?,
        ReadStatus::Complete
    ) {
        return if running.is_some_and(|running| !running.load(Ordering::Acquire)) {
            Ok(None)
        } else {
            Err(ProtocolError::Frame("truncated body"))
        };
    }
    let mut sentinel = [0_u8; 5];
    if !matches!(
        read_exact(reader, &mut sentinel, running)?,
        ReadStatus::Complete
    ) {
        return if running.is_some_and(|running| !running.load(Ordering::Acquire)) {
            Ok(None)
        } else {
            Err(ProtocolError::Frame("truncated sentinel"))
        };
    }
    if &sentinel != SENTINEL {
        return Err(ProtocolError::Frame("sentinel mismatch"));
    }
    Ok(Some(body))
}

fn read_exact(
    reader: &mut impl Read,
    buffer: &mut [u8],
    running: Option<&AtomicBool>,
) -> Result<ReadStatus, ProtocolError> {
    let mut offset = 0;
    while offset < buffer.len() {
        match reader.read(&mut buffer[offset..]) {
            Ok(0) if offset == 0 => return Ok(ReadStatus::Eof),
            Ok(0) => return Err(ProtocolError::Frame("truncated frame")),
            Ok(read) => offset += read,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error)
                if running.is_some()
                    && matches!(
                        error.kind(),
                        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                    ) =>
            {
                if !running.expect("checked as present").load(Ordering::Acquire) {
                    return Ok(ReadStatus::Stopped);
                }
            }
            Err(error) => return Err(error.into()),
        }
    }
    Ok(ReadStatus::Complete)
}

pub(crate) fn write_frame(writer: &mut impl Write, body: &[u8]) -> Result<(), ProtocolError> {
    if body.is_empty() {
        return Err(ProtocolError::Frame("zero-length body"));
    }
    if body.len() > MAX_FRAME_BYTES {
        return Err(ProtocolError::Frame("body exceeds the frame limit"));
    }
    let length = u32::try_from(body.len()).map_err(|_| ProtocolError::Frame("body too large"))?;
    writer.write_all(&length.to_be_bytes())?;
    writer.write_all(body)?;
    writer.write_all(SENTINEL)?;
    writer.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::{self, Cursor};
    use std::sync::atomic::AtomicBool;

    use super::*;

    #[test]
    fn round_trip_and_integrity_checks() {
        let mut encoded = Vec::new();
        write_frame(&mut encoded, "你好".as_bytes()).unwrap();
        assert_eq!(
            read_frame(&mut encoded.as_slice()).unwrap(),
            Some("你好".as_bytes().to_vec())
        );

        *encoded.last_mut().unwrap() = b'!';
        assert!(matches!(
            read_frame(&mut encoded.as_slice()),
            Err(ProtocolError::Frame("sentinel mismatch"))
        ));
    }

    struct Fragmented {
        inner: Cursor<Vec<u8>>,
    }

    impl Read for Fragmented {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            let maximum = buffer.len().min(1);
            self.inner.read(&mut buffer[..maximum])
        }
    }

    #[test]
    fn interruptible_reader_preserves_fragmented_frames() {
        let mut encoded = Vec::new();
        write_frame(&mut encoded, b"fragmented").unwrap();
        let running = AtomicBool::new(true);
        let mut reader = Fragmented {
            inner: Cursor::new(encoded),
        };
        assert_eq!(
            read_frame_interruptible(&mut reader, &running).unwrap(),
            Some(b"fragmented".to_vec())
        );
    }
}
