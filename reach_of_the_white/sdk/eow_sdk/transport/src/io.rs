//! Connection-independent framed adapters over `std::io::Read` and `Write`.

use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::time::Instant;

use crate::error::FramedIoError;
use crate::frame::{self, Deframer};

const DEFAULT_READ_CHUNK: usize = 8 * 1024;

/// Reads complete frame bodies without surfacing partial data.
#[derive(Debug)]
pub struct FramedReader<R> {
    inner: R,
    deframer: Deframer,
    chunk: Vec<u8>,
}

impl<R: Read> FramedReader<R> {
    pub fn new(inner: R) -> Self {
        Self::with_chunk_size(inner, DEFAULT_READ_CHUNK)
    }

    pub fn with_chunk_size(inner: R, chunk_size: usize) -> Self {
        Self {
            inner,
            deframer: Deframer::new(),
            chunk: vec![0; chunk_size.max(1)],
        }
    }

    pub fn read_frame(&mut self) -> Result<Option<Vec<u8>>, FramedIoError> {
        loop {
            if let Some(body) = self
                .deframer
                .next_frame()
                .map_err(FramedIoError::FrameAbort)?
            {
                return Ok(Some(body));
            }

            let read = self.inner.read(&mut self.chunk)?;
            if read == 0 {
                return if self.deframer.buffered_len() == 0 {
                    Ok(None)
                } else {
                    Err(FramedIoError::TruncatedFrame)
                };
            }
            self.deframer
                .push(&self.chunk[..read])
                .map_err(FramedIoError::FrameAbort)?;
        }
    }

    pub fn get_ref(&self) -> &R {
        &self.inner
    }

    pub fn get_mut(&mut self) -> &mut R {
        &mut self.inner
    }

    pub fn into_inner(self) -> R {
        self.inner
    }
}

impl FramedReader<TcpStream> {
    /// Read one complete frame under one absolute deadline. Partial reads do not
    /// restart the deadline, and any bytes beyond the first frame remain buffered.
    pub fn read_frame_until(
        &mut self,
        deadline: Instant,
    ) -> Result<Option<Vec<u8>>, FramedIoError> {
        loop {
            if let Some(body) = self
                .deframer
                .next_frame()
                .map_err(FramedIoError::FrameAbort)?
            {
                return Ok(Some(body));
            }

            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(FramedIoError::Io(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "handshake deadline elapsed",
                )));
            }
            self.inner.set_read_timeout(Some(remaining))?;
            let read = self.inner.read(&mut self.chunk)?;
            if read == 0 {
                return if self.deframer.buffered_len() == 0 {
                    Ok(None)
                } else {
                    Err(FramedIoError::TruncatedFrame)
                };
            }
            self.deframer
                .push(&self.chunk[..read])
                .map_err(FramedIoError::FrameAbort)?;
        }
    }
}

/// Writes one complete protocol frame per call.
#[derive(Debug)]
pub struct FramedWriter<W> {
    inner: W,
}

impl<W: Write> FramedWriter<W> {
    pub fn new(inner: W) -> Self {
        Self { inner }
    }

    pub fn write_frame(&mut self, body: &[u8]) -> Result<(), FramedIoError> {
        let framed = frame::encode(body)?;
        self.inner.write_all(&framed)?;
        Ok(())
    }

    pub fn flush(&mut self) -> Result<(), FramedIoError> {
        self.inner.flush()?;
        Ok(())
    }

    pub fn get_ref(&self) -> &W {
        &self.inner
    }

    pub fn get_mut(&mut self) -> &mut W {
        &mut self.inner
    }

    pub fn into_inner(self) -> W {
        self.inner
    }
}
