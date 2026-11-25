use std::{net::{Ipv4Addr, SocketAddrV4}, sync::{Arc, Mutex}, time::Duration};

use serde::Deserialize;
use serde_json::{Value, json};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use tokio::{net::TcpStream, time::timeout};

use crate::transport::SdkError;
use crate::transport::envelope::{Envelope, EnvelopeKind, EnvelopePayload, EnvelopeType, SCHEMA};
use crate::transport::tcp_connection::TcpConnection;
use crate::transport::tcp_framing;

const EVENT_SUFFIXES: usize = 100;

#[derive(Deserialize)]
struct HandshakeReply {
    handshake: HandshakeResult,
}

#[derive(Deserialize)]
struct HandshakeResult {
    result: String,
    reason: Option<String>,
}

struct EventIds {
    millisecond: Option<i128>,
    used: [bool; EVENT_SUFFIXES],
}

impl EventIds {

    fn next(&mut self, now: OffsetDateTime) -> Result<String, SdkError> {
        let millisecond: i128 = now.unix_timestamp_nanos().div_euclid(1_000_000);
        if self.millisecond != Some(millisecond) {
            self.millisecond = Some(millisecond);
            self.used.fill(false);
        }
        let mut available = self.used.iter().enumerate().filter_map(|(suffix, used): (usize, &bool)| (!*used).then_some(suffix));
        let count: usize = available.clone().count();
        if count == 0 {
            return Err(SdkError::EventIdExhausted);
        }
        let suffix: usize = available.nth(rand::random_range(0..count)).expect("selected suffix is available");
        self.used[suffix] = true;
        Ok(format!("7{:02}{:02}{:02}{:03}{suffix:02}", now.hour(), now.minute(), now.second(), now.millisecond()))
    }

}

struct Session {
    connection: Arc<TcpConnection>,
    ready: bool,
}

pub struct SdkTransport {
    sdk_id: String,
    port: u16,
    timeout: Duration,
    session: Mutex<Option<Session>>,
    event_ids: Mutex<EventIds>,
}

impl SdkTransport {

    pub fn new(sdk_id: String, port: u16, timeout: Duration) -> Self {
        Self {
            sdk_id,
            port,
            timeout,
            session: Mutex::new(None),
            event_ids: Mutex::new(EventIds { millisecond: None, used: [false; EVENT_SUFFIXES] }),
        }
    }

    pub fn sdk_id(&self) -> &str {
        &self.sdk_id
    }

    pub fn connection(&self) -> Result<Arc<TcpConnection>, SdkError> {
        let session = self.session.lock().unwrap();
        let session: &Session = session.as_ref().filter(|session: &&Session| session.ready).ok_or(SdkError::Disconnected)?;
        session.connection.ensure_open()?;
        Ok(Arc::clone(&session.connection))
    }

    pub async fn connect(&self) -> Result<Arc<TcpConnection>, SdkError> {
        let previous: Option<Arc<TcpConnection>> = self.session.lock().unwrap().as_ref().map(|session: &Session| Arc::clone(&session.connection));
        if let Some(connection) = previous {
            if connection.ensure_open().is_ok() {
                return Err(SdkError::AlreadyConnected);
            }
            self.finish_close(&connection).await;
        }

        let stream: TcpStream = timeout(self.timeout, async {
            let address: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::LOCALHOST, self.port);
            let mut stream: TcpStream = TcpStream::connect(address).await?;
            let request: Vec<u8> = serde_json::to_vec(&json!({
                "handshake": { "schema": SCHEMA, "sdk_id": self.sdk_id() }
            }))?;
            tcp_framing::write_frame(&mut stream, &request).await?;
            let reply: Vec<u8> = tcp_framing::read_frame(&mut stream).await?.ok_or(SdkError::Disconnected)?;
            let reply: HandshakeReply = serde_json::from_slice(&reply)?;
            match reply.handshake.result.as_str() {
                "accept" => Ok(stream),
                "deny" => Err(SdkError::HandshakeDenied(reply.handshake.reason)),
                _ => Err(SdkError::Protocol("unknown handshake result")),
            }
        }).await.map_err(|_| SdkError::Timeout)??;

        let connection: Arc<TcpConnection> = Arc::new(TcpConnection::new(stream, self.timeout));
        *self.session.lock().unwrap() = Some(Session { connection: Arc::clone(&connection), ready: false });
        Ok(connection)
    }

    pub fn mark_ready(&self) -> Result<(), SdkError> {
        let mut session = self.session.lock().unwrap();
        let session: &mut Session = session.as_mut().ok_or(SdkError::Disconnected)?;
        session.connection.ensure_open()?;
        session.ready = true;
        Ok(())
    }

    pub async fn finish_close(&self, connection: &TcpConnection) {
        connection.wait_closed().await;
        self.session.lock().unwrap().take();
    }

    pub fn envelope_build(&self, envelope_type: EnvelopeType, data: Option<Value>, correlation_id: Option<&str>, revision: Option<i32>) -> Result<Envelope, SdkError> {
        let (envelope_kind, payload_type, kind): (EnvelopeKind, &str, Option<&str>) = envelope_type.fields();
        let (now, event_id): (OffsetDateTime, String) = {
            let mut event_ids = self.event_ids.lock().unwrap();
            let now: OffsetDateTime = OffsetDateTime::now_utc();
            (now, event_ids.next(now)?)
        };
        let correlation_id: Option<String> = if envelope_kind == EnvelopeKind::Response {
            correlation_id.map(str::to_owned)
        }
        else {
            None
        };
        let ts_utc: String = now.format(&Rfc3339).map_err(|_| SdkError::InvalidData("Could not format the SDK timestamp".into()))?;

        Ok(Envelope {
            envelope_schema: SCHEMA.into(),
            envelope_kind,
            source: "eow_sdk".into(),
            target: "eow".into(),
            event_id,
            correlation_id,
            ts_utc,
            payload: EnvelopePayload { sdk_id: self.sdk_id.clone(), payload_type: payload_type.into(), kind: kind.map(str::to_owned), revision, data },
        })
    }

}
