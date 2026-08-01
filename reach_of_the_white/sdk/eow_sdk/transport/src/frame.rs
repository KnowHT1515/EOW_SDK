//! Content-agnostic TCP framing and incremental deframing.

use crate::error::FramedIoError;
use crate::limits::{MAX_FRAME_BODY, SENTINEL};

/// Why a framed byte stream can no longer be trusted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameAbort {
    ZeroLength,
    Oversize,
    BadSentinel,
}

/// Result of decoding the front of a byte buffer.
#[derive(Debug, PartialEq, Eq)]
pub enum DecodeResult<'a> {
    Frame { body: &'a [u8], consumed: usize },
    NeedMore,
    Abort(FrameAbort),
}

/// Encode a non-empty body as one protocol frame.
pub fn encode(body: &[u8]) -> Result<Vec<u8>, FramedIoError> {
    if body.is_empty() {
        return Err(FramedIoError::EmptyBody);
    }
    if body.len() > MAX_FRAME_BODY {
        return Err(FramedIoError::BodyOversize {
            actual: body.len(),
            maximum: MAX_FRAME_BODY,
        });
    }

    let mut framed = Vec::with_capacity(4 + body.len() + SENTINEL.len());
    framed.extend_from_slice(&(body.len() as u32).to_be_bytes());
    framed.extend_from_slice(body);
    framed.extend_from_slice(&SENTINEL);
    Ok(framed)
}

/// Decode at most one complete frame from the front of `input`.
pub fn decode(input: &[u8]) -> DecodeResult<'_> {
    if input.len() < 4 {
        return DecodeResult::NeedMore;
    }

    let body_len = u32::from_be_bytes([input[0], input[1], input[2], input[3]]) as usize;
    if body_len == 0 {
        return DecodeResult::Abort(FrameAbort::ZeroLength);
    }
    if body_len > MAX_FRAME_BODY {
        return DecodeResult::Abort(FrameAbort::Oversize);
    }

    let total = 4 + body_len + SENTINEL.len();
    if input.len() < total {
        return DecodeResult::NeedMore;
    }

    let body = &input[4..4 + body_len];
    if input[4 + body_len..total] != SENTINEL {
        return DecodeResult::Abort(FrameAbort::BadSentinel);
    }

    DecodeResult::Frame {
        body,
        consumed: total,
    }
}

/// Incremental frame decoder that permanently remembers an abort.
#[derive(Debug, Default)]
pub struct Deframer {
    buffer: Vec<u8>,
    aborted: Option<FrameAbort>,
}

impl Deframer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add bytes received from the stream.
    pub fn push(&mut self, bytes: &[u8]) -> Result<(), FrameAbort> {
        if let Some(reason) = self.aborted {
            return Err(reason);
        }
        self.buffer.extend_from_slice(bytes);
        Ok(())
    }

    /// Remove and return the next complete body, if one is buffered.
    pub fn next_frame(&mut self) -> Result<Option<Vec<u8>>, FrameAbort> {
        if let Some(reason) = self.aborted {
            return Err(reason);
        }

        match decode(&self.buffer) {
            DecodeResult::Frame { body, consumed } => {
                let body = body.to_vec();
                self.buffer.drain(..consumed);
                Ok(Some(body))
            }
            DecodeResult::NeedMore => Ok(None),
            DecodeResult::Abort(reason) => {
                self.aborted = Some(reason);
                Err(reason)
            }
        }
    }

    pub fn buffered_len(&self) -> usize {
        self.buffer.len()
    }

    pub fn is_aborted(&self) -> bool {
        self.aborted.is_some()
    }
}
