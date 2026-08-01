use std::fmt;
use std::io;
use std::net::{SocketAddr, TcpStream};
use std::sync::Arc;

use crate::envelope::{Envelope, EnvelopeError, EnvelopeKind, EventMetadataGenerator};
use crate::frame::{self, FrameError};
use crate::handshake::{self, HandshakeError, HandshakeRequest};

const MAX_LOGGED_JSON_CHARS: usize = 4 * 1024;

pub fn run(
    mut stream: TcpStream,
    peer: SocketAddr,
    instance: &str,
    metadata: Arc<EventMetadataGenerator>,
) -> Result<(), SessionError> {
    stream.set_nodelay(true).map_err(SessionError::Socket)?;

    let Some(handshake_body) = frame::read_frame(&mut stream)? else {
        println!("[CLOSE] peer={peer} reason=disconnected_before_handshake");
        return Ok(());
    };
    let request = HandshakeRequest::parse(&handshake_body)?;

    if !request.is_acceptable() {
        frame::write_frame(&mut stream, &handshake::deny_body())?;
        println!(
            "[HANDSHAKE] peer={peer} sdk_id={:?} schema={:?} result=deny",
            request.id, request.schema
        );
        println!("[CLOSE] peer={peer} reason=handshake_denied");
        return Ok(());
    }

    frame::write_frame(&mut stream, &handshake::accept_body(instance))?;
    println!(
        "[HANDSHAKE] peer={peer} sdk_id={:?} schema={:?} result=accept instance={instance}",
        request.id, request.schema
    );

    let ready = Envelope::ready(instance, &metadata);
    send_envelope(&mut stream, peer, &ready)?;

    loop {
        let Some(body) = frame::read_frame(&mut stream)? else {
            println!("[CLOSE] peer={peer} reason=client_disconnected");
            return Ok(());
        };
        let envelope = Envelope::parse(&body)?;
        envelope.validate_inbound_for_eow()?;
        log_envelope("RECV", peer, &envelope);

        if envelope.kind() == EnvelopeKind::Request {
            let response = Envelope::echo_response(&envelope, &metadata)?;
            send_envelope(&mut stream, peer, &response)?;
        }
    }
}

fn send_envelope(
    stream: &mut TcpStream,
    peer: SocketAddr,
    envelope: &Envelope,
) -> Result<(), SessionError> {
    let body = envelope.to_json_vec();
    frame::write_frame(stream, &body)?;
    log_envelope("SEND", peer, envelope);
    Ok(())
}

fn log_envelope(direction: &str, peer: SocketAddr, envelope: &Envelope) {
    let correlation = envelope.correlation_id().unwrap_or("null");
    let json = envelope.to_json_vec();
    println!(
        "[{direction}] peer={peer} kind={} event_id={} correlation_id={} json={}",
        envelope.kind().as_str(),
        envelope.event_id(),
        correlation,
        compact_log_json(&json)
    );
}

fn compact_log_json(body: &[u8]) -> String {
    let text = String::from_utf8_lossy(body);
    let mut chars = text.chars();
    let displayed: String = chars.by_ref().take(MAX_LOGGED_JSON_CHARS).collect();
    if chars.next().is_some() {
        format!("{displayed}...<truncated>")
    } else {
        displayed
    }
}

#[derive(Debug)]
pub enum SessionError {
    Socket(io::Error),
    Frame(FrameError),
    Handshake(HandshakeError),
    Envelope(EnvelopeError),
}

impl fmt::Display for SessionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Socket(error) => write!(formatter, "socket setup failed: {error}"),
            Self::Frame(error) => write!(formatter, "frame stage failed: {error}"),
            Self::Handshake(error) => write!(formatter, "handshake stage failed: {error}"),
            Self::Envelope(error) => write!(formatter, "Envelope stage failed: {error}"),
        }
    }
}

impl std::error::Error for SessionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Socket(error) => Some(error),
            Self::Frame(error) => Some(error),
            Self::Handshake(error) => Some(error),
            Self::Envelope(error) => Some(error),
        }
    }
}

impl From<FrameError> for SessionError {
    fn from(error: FrameError) -> Self {
        Self::Frame(error)
    }
}

impl From<HandshakeError> for SessionError {
    fn from(error: HandshakeError) -> Self {
        Self::Handshake(error)
    }
}

impl From<EnvelopeError> for SessionError {
    fn from(error: EnvelopeError) -> Self {
        Self::Envelope(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_json_is_truncated_on_a_character_boundary() {
        let json = format!("{{\"value\":\"{}?\"}}", "x".repeat(MAX_LOGGED_JSON_CHARS));
        let logged = compact_log_json(json.as_bytes());
        assert!(logged.ends_with("...<truncated>"));
        assert!(std::str::from_utf8(logged.as_bytes()).is_ok());
    }

    #[test]
    fn short_json_is_unchanged() {
        assert_eq!(compact_log_json(br#"{"ok":true}"#), r#"{"ok":true}"#);
    }
}
