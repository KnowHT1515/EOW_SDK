//! Process-wide multi-instance TCP client runtime.

use std::collections::HashMap;
use std::net::{Shutdown, SocketAddrV4, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::context::{InboundSink, context};
use crate::endpoint::InstanceRegistry;
use crate::envelope::SdkEnvelope;
use crate::error::{ConnectError, DisconnectError, EnvelopeError};
use crate::handshake::{HandshakeRequest, HandshakeResponse};
use crate::io::{FramedReader, FramedWriter};
use crate::limits::{DEFAULT_HANDSHAKE_TIMEOUT, HANDSHAKE_SCHEMA};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendResult {
    pub ok: bool,
    pub violation: Vec<String>,
}

impl SendResult {
    fn accepted() -> Self {
        Self {
            ok: true,
            violation: Vec::new(),
        }
    }

    fn rejected(violation: Vec<String>) -> Self {
        Self {
            ok: false,
            violation,
        }
    }
}

#[derive(Debug)]
enum EnqueueError {
    Unavailable,
}

enum WriterCommand {
    Body(Vec<u8>),
    Close,
}

struct ConnectionHandle {
    instance: String,
    sender: Sender<WriterCommand>,

    alive: Arc<AtomicBool>,
    control: TcpStream,
    writer: Option<JoinHandle<()>>,
    reader: Option<JoinHandle<()>>,
}

impl ConnectionHandle {
    fn enqueue(&self, body: Vec<u8>) -> Result<(), EnqueueError> {
        if !self.alive.load(Ordering::Acquire) {
            return Err(EnqueueError::Unavailable);
        }

        if self.sender.send(WriterCommand::Body(body)).is_err() {
            return Err(EnqueueError::Unavailable);
        }
        Ok(())
    }

    fn close(mut self) -> Result<(), DisconnectError> {
        self.alive.store(false, Ordering::Release);
        let _ = self.sender.send(WriterCommand::Close);

        let writer_panicked = self
            .writer
            .take()
            .is_some_and(|worker| worker.join().is_err());
        let _ = self.control.shutdown(Shutdown::Both);
        let reader_panicked = self
            .reader
            .take()
            .is_some_and(|worker| worker.join().is_err());

        if writer_panicked || reader_panicked {
            Err(DisconnectError::WorkerPanicked(self.instance))
        } else {
            Ok(())
        }
    }
}

pub(crate) struct TransportRuntime {
    connections: Mutex<HashMap<String, ConnectionHandle>>,
    inbound: InboundSink,
}

impl TransportRuntime {
    pub(crate) fn new(inbound: InboundSink) -> Self {
        Self {
            connections: Mutex::new(HashMap::new()),
            inbound,
        }
    }

    fn connect(
        &self,
        sdk_id: &str,
        registry: &InstanceRegistry,
        instance: &str,
        address: SocketAddrV4,
        handshake_timeout: Duration,
    ) -> Result<(), ConnectError> {
        let stale = {
            let mut connections = self
                .connections
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if connections
                .get(instance)
                .is_some_and(|connection| connection.alive.load(Ordering::Acquire))
            {
                return Err(ConnectError::AlreadyConnected(instance.to_owned()));
            }
            connections.remove(instance)
        };
        if let Some(stale) = stale {
            let _ = stale.close();
        }

        let connection = establish_connection(
            sdk_id,
            registry.clone(),
            instance,
            address,
            handshake_timeout,
            self.inbound.clone(),
        )?;

        let mut connections = self
            .connections
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if connections
            .get(instance)
            .is_some_and(|existing| existing.alive.load(Ordering::Acquire))
        {
            let _ = connection.close();
            return Err(ConnectError::AlreadyConnected(instance.to_owned()));
        }
        connections.insert(instance.to_owned(), connection);
        Ok(())
    }

    fn disconnect(&self, instance: &str) -> Result<(), DisconnectError> {
        let connection = self
            .connections
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(instance)
            .ok_or_else(|| DisconnectError::NotConnected(instance.to_owned()))?;
        connection.close()
    }

    fn enqueue(&self, target: &str, body: Vec<u8>) -> Result<(), EnqueueError> {
        let connections = self
            .connections
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        connections
            .get(target)
            .ok_or(EnqueueError::Unavailable)?
            .enqueue(body)
    }
}

pub fn connect(instance: &str) -> Result<(), ConnectError> {
    connect_timeout(instance, DEFAULT_HANDSHAKE_TIMEOUT)
}

pub fn connect_timeout(instance: &str, timeout: Duration) -> Result<(), ConnectError> {
    let context = context().ok_or(ConnectError::NotInitialized)?;
    let endpoint = context.registry.resolve(instance)?;
    context.runtime.connect(
        &context.sdk_id,
        &context.registry,
        instance,
        endpoint.address,
        timeout,
    )
}

#[doc(hidden)]
pub fn connect_at(instance: &str, address: SocketAddrV4) -> Result<(), ConnectError> {
    connect_at_timeout(instance, address, DEFAULT_HANDSHAKE_TIMEOUT)
}

#[doc(hidden)]
pub fn connect_at_timeout(
    instance: &str,
    address: SocketAddrV4,
    timeout: Duration,
) -> Result<(), ConnectError> {
    let context = context().ok_or(ConnectError::NotInitialized)?;
    let endpoint = context
        .registry
        .resolve_with_override(instance, Some(address))?;
    context.runtime.connect(
        &context.sdk_id,
        &context.registry,
        instance,
        endpoint.address,
        timeout,
    )
}

pub fn disconnect(instance: &str) -> Result<(), DisconnectError> {
    let context = context().ok_or(DisconnectError::NotInitialized)?;
    context.runtime.disconnect(instance)
}

pub fn send(sdk_envelope_json: String) -> SendResult {
    let Some(context) = context() else {
        return SendResult::rejected(Vec::new());
    };

    let envelope =
        match SdkEnvelope::parse_with_registry(sdk_envelope_json.as_bytes(), &context.registry) {
            Ok(envelope) => envelope,
            Err(EnvelopeError::Invalid(violations)) => {
                return SendResult::rejected(
                    violations
                        .into_iter()
                        .map(|violation| violation.as_str().to_owned())
                        .collect(),
                );
            }
            Err(EnvelopeError::Oversize { .. }) => {
                return SendResult::rejected(vec!["<oversize>".to_owned()]);
            }
            Err(EnvelopeError::Json(_)) => {
                return SendResult::rejected(vec!["<malformed_json>".to_owned()]);
            }
            Err(EnvelopeError::Config(_)) => {
                return SendResult::rejected(vec!["<registry>".to_owned()]);
            }
        };

    let violations = envelope.outbound_violations(&context.sdk_id, &context.registry);
    if !violations.is_empty() {
        return SendResult::rejected(violations);
    }

    let target = envelope.target().to_owned();
    match context
        .runtime
        .enqueue(&target, sdk_envelope_json.into_bytes())
    {
        Ok(()) => SendResult::accepted(),

        Err(EnqueueError::Unavailable) => SendResult::rejected(Vec::new()),
    }
}

fn establish_connection(
    sdk_id: &str,
    registry: InstanceRegistry,
    instance: &str,
    address: SocketAddrV4,
    handshake_timeout: Duration,
    inbound: InboundSink,
) -> Result<ConnectionHandle, ConnectError> {
    let deadline = Instant::now()
        .checked_add(handshake_timeout)
        .ok_or(ConnectError::HandshakeTimeout)?;
    let socket_address = address.into();
    let mut stream = TcpStream::connect_timeout(&socket_address, remaining_until(deadline)?)
        .map_err(handshake_io_error)?;
    stream.set_nodelay(true).map_err(ConnectError::Io)?;
    stream
        .set_write_timeout(Some(remaining_until(deadline)?))
        .map_err(ConnectError::Io)?;

    {
        let mut writer = FramedWriter::new(&mut stream);
        writer
            .write_frame(&HandshakeRequest::new(sdk_id).to_body())
            .map_err(handshake_framed_error)?;
        writer.flush().map_err(handshake_framed_error)?;
    }

    let mut framed_reader = FramedReader::new(stream.try_clone().map_err(ConnectError::Io)?);
    let response_body = framed_reader
        .read_frame_until(deadline)
        .map_err(handshake_framed_error)?
        .ok_or(ConnectError::HandshakeClosed)?;
    let response = HandshakeResponse::parse(&response_body)?;
    let schema = match &response {
        HandshakeResponse::Accept { schema, .. } | HandshakeResponse::Deny { schema } => schema,
    };
    if schema != HANDSHAKE_SCHEMA {
        return Err(ConnectError::UnsupportedSchema(schema.clone()));
    }
    match response {
        HandshakeResponse::Deny { .. } => return Err(ConnectError::HandshakeDenied),
        HandshakeResponse::Accept {
            instance: actual, ..
        } if actual != instance => {
            return Err(ConnectError::InstanceMismatch {
                expected: instance.to_owned(),
                actual,
            });
        }
        HandshakeResponse::Accept { .. } => {}
    }

    stream.set_read_timeout(None).map_err(ConnectError::Io)?;
    stream.set_write_timeout(None).map_err(ConnectError::Io)?;
    framed_reader
        .get_mut()
        .set_read_timeout(None)
        .map_err(ConnectError::Io)?;
    framed_reader
        .get_mut()
        .set_write_timeout(None)
        .map_err(ConnectError::Io)?;

    let write_stream = stream.try_clone().map_err(ConnectError::Io)?;
    let control = stream;
    let (writer_tx, writer_rx) = mpsc::channel();

    let alive = Arc::new(AtomicBool::new(true));

    let writer_alive = Arc::clone(&alive);
    let writer_instance = instance.to_owned();
    let writer = thread::Builder::new()
        .name(format!("eow-sdk-writer-{writer_instance}"))
        .spawn(move || {
            let mut framed = FramedWriter::new(write_stream);
            while let Ok(command) = writer_rx.recv() {
                match command {
                    WriterCommand::Body(body) => {
                        if framed.write_frame(&body).is_err() {
                            break;
                        }
                    }
                    WriterCommand::Close => {
                        let _ = framed.flush();
                        break;
                    }
                }
            }
            writer_alive.store(false, Ordering::Release);
            let stream = framed.into_inner();
            let _ = stream.shutdown(Shutdown::Both);
        })
        .map_err(ConnectError::Spawn)?;

    let reader_alive = Arc::clone(&alive);

    let reader_sender = writer_tx.clone();
    let reader_sdk_id = sdk_id.to_owned();
    let reader_instance = instance.to_owned();
    let reader = match thread::Builder::new()
        .name(format!("eow-sdk-reader-{reader_instance}"))
        .spawn(move || {
            let mut framed = framed_reader;
            while let Ok(Some(body)) = framed.read_frame() {
                let envelope = match SdkEnvelope::parse_with_registry(&body, &registry) {
                    Ok(envelope) => envelope,
                    Err(_) => break,
                };
                if !envelope
                    .inbound_violations(&reader_sdk_id, &reader_instance, &registry)
                    .is_empty()
                {
                    break;
                }

                if inbound.send(envelope).is_err() {
                    break;
                }
            }
            reader_alive.store(false, Ordering::Release);
            let _ = reader_sender.send(WriterCommand::Close);
        }) {
        Ok(reader) => reader,
        Err(error) => {
            let _ = writer_tx.send(WriterCommand::Close);
            let _ = writer.join();
            return Err(ConnectError::Spawn(error));
        }
    };

    Ok(ConnectionHandle {
        instance: instance.to_owned(),
        sender: writer_tx,

        alive,
        control,
        writer: Some(writer),
        reader: Some(reader),
    })
}

fn remaining_until(deadline: Instant) -> Result<Duration, ConnectError> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        Err(ConnectError::HandshakeTimeout)
    } else {
        Ok(remaining)
    }
}

fn handshake_io_error(error: std::io::Error) -> ConnectError {
    if matches!(
        error.kind(),
        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
    ) {
        ConnectError::HandshakeTimeout
    } else {
        ConnectError::Io(error)
    }
}

fn handshake_framed_error(error: crate::error::FramedIoError) -> ConnectError {
    match error {
        crate::error::FramedIoError::Io(error) => handshake_io_error(error),
        other => ConnectError::Framed(other),
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::net::{Ipv4Addr, SocketAddr, TcpListener};
    use std::sync::mpsc;
    use std::time::Duration;

    use serde_json::json;

    use super::*;
    use crate::frame;

    fn envelope(
        kind: &str,
        source: &str,
        target: &str,
        event_id: &str,
        correlation_id: Option<&str>,
        sdk_id: &str,
    ) -> SdkEnvelope {
        SdkEnvelope::try_from_value(json!({
            "envelope_schema": "1.0",
            "envelope_kind": kind,
            "source": source,
            "target": target,
            "event_id": event_id,
            "correlation_id": correlation_id,
            "payload": {"sdk_id": sdk_id, "type": "test"}
        }))
        .unwrap()
    }

    fn handshake_peer(response: Option<Vec<u8>>, hold_open: Duration) -> SocketAddrV4 {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let address = match listener.local_addr().unwrap() {
            SocketAddr::V4(address) => address,
            SocketAddr::V6(_) => unreachable!("test listener is IPv4"),
        };
        thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = FramedReader::new(stream.try_clone().unwrap());
            if reader.read_frame().ok().flatten().is_none() {
                return;
            }
            if let Some(response) = response {
                let _ = (&stream).write_all(&response);
            }
            thread::sleep(hold_open);
        });
        address
    }

    fn accepted_handshake(instance: &str) -> Vec<u8> {
        frame::encode(&HandshakeResponse::accept(instance).to_body()).unwrap()
    }

    fn wait_until_closed(connection: &ConnectionHandle) {
        let deadline = Instant::now() + Duration::from_millis(500);
        while connection.alive.load(Ordering::Acquire) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        assert!(!connection.alive.load(Ordering::Acquire));
    }

    #[test]
    fn silent_and_partial_handshakes_share_one_absolute_deadline() {
        for response in [None, Some(accepted_handshake("silvy")[..3].to_vec())] {
            let address = handshake_peer(response, Duration::from_millis(300));
            let (inbound, _bucket) = mpsc::channel();
            let started = Instant::now();
            let result = establish_connection(
                "Cyberpunk",
                InstanceRegistry::bundled().unwrap(),
                "silvy",
                address,
                Duration::from_millis(80),
                inbound,
            );
            assert!(matches!(result, Err(ConnectError::HandshakeTimeout)));
            assert!(
                started.elapsed() < Duration::from_millis(250),
                "handshake exceeded bounded tolerance: {:?}",
                started.elapsed()
            );
        }
    }

    #[test]
    fn accepted_handshake_clears_temporary_socket_timeouts() {
        let address = handshake_peer(
            Some(accepted_handshake("silvy")),
            Duration::from_millis(300),
        );
        let (inbound, _bucket) = mpsc::channel();
        let connection = establish_connection(
            "Cyberpunk",
            InstanceRegistry::bundled().unwrap(),
            "silvy",
            address,
            Duration::from_millis(80),
            inbound,
        )
        .unwrap();

        thread::sleep(Duration::from_millis(160));
        assert!(connection.alive.load(Ordering::Acquire));
        connection.close().unwrap();
    }

    #[test]
    fn handshake_reader_preserves_prefetched_post_handshake_frame() {
        let ready = json!({
            "envelope_schema": "1.0",
            "envelope_kind": "event",
            "source": "silvy",
            "target": "eow_sdk",
            "event_id": "712345678905",
            "correlation_id": null,
            "payload": {"sdk_id": "Cyberpunk", "type": "ready"}
        });
        let mut response = accepted_handshake("silvy");
        response.extend(frame::encode(&serde_json::to_vec(&ready).unwrap()).unwrap());
        let address = handshake_peer(Some(response), Duration::from_millis(300));
        let (inbound, bucket) = mpsc::channel();
        let connection = establish_connection(
            "Cyberpunk",
            InstanceRegistry::bundled().unwrap(),
            "silvy",
            address,
            Duration::from_millis(100),
            inbound,
        )
        .unwrap();

        let received = bucket.recv_timeout(Duration::from_millis(200)).unwrap();
        assert_eq!(received.event_id(), "712345678905");
        connection.close().unwrap();
    }

    #[test]
    fn reader_rejects_cross_connection_identity_and_sdk_id() {
        let invalid_envelopes = [
            json!({
                "envelope_schema": "1.0",
                "envelope_kind": "event",
                "source": "rucia",
                "target": "eow_sdk",
                "event_id": "712345678906",
                "correlation_id": null,
                "payload": {"sdk_id": "Cyberpunk", "type": "spoof"}
            }),
            json!({
                "envelope_schema": "1.0",
                "envelope_kind": "event",
                "source": "silvy",
                "target": "eow_sdk",
                "event_id": "712345678907",
                "correlation_id": null,
                "payload": {"sdk_id": "Other", "type": "spoof"}
            }),
        ];

        for invalid in invalid_envelopes {
            let mut response = accepted_handshake("silvy");
            response.extend(frame::encode(&serde_json::to_vec(&invalid).unwrap()).unwrap());
            let address = handshake_peer(Some(response), Duration::from_millis(300));
            let (inbound, bucket) = mpsc::channel::<SdkEnvelope>();
            let connection = establish_connection(
                "Cyberpunk",
                InstanceRegistry::bundled().unwrap(),
                "silvy",
                address,
                Duration::from_millis(100),
                inbound,
            )
            .unwrap();

            wait_until_closed(&connection);
            assert!(matches!(
                bucket.try_recv(),
                Err(mpsc::TryRecvError::Empty | mpsc::TryRecvError::Disconnected)
            ));
            connection.close().unwrap();
        }
    }

    #[test]
    fn failed_enqueue_is_reported_as_unavailable() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let peer = thread::spawn(move || listener.accept().unwrap().0);
        let control = TcpStream::connect(address).unwrap();
        let peer_stream = peer.join().unwrap();

        let (sender, receiver) = mpsc::channel();
        drop(receiver);
        let connection = ConnectionHandle {
            instance: "silvy".to_owned(),
            sender,
            alive: Arc::new(AtomicBool::new(true)),
            control,
            writer: None,
            reader: None,
        };
        let request = envelope(
            "request",
            "eow_sdk",
            "silvy",
            "712345678912",
            None,
            "Cyberpunk",
        );
        assert!(matches!(
            connection.enqueue(request.to_json_vec()),
            Err(EnqueueError::Unavailable)
        ));
        drop(peer_stream);
        connection.close().unwrap();
    }

    #[test]
    fn reader_accepts_unknown_and_repeated_response_correlations() {
        let response_envelope = json!({
            "envelope_schema": "1.0",
            "envelope_kind": "response",
            "source": "silvy",
            "target": "eow_sdk",
            "event_id": "712345678908",
            "correlation_id": "712345678900",
            "payload": {"sdk_id": "Cyberpunk", "type": "ack"}
        });
        let mut response = accepted_handshake("silvy");
        let frame = frame::encode(&serde_json::to_vec(&response_envelope).unwrap()).unwrap();
        response.extend(&frame);
        response.extend(frame);
        let address = handshake_peer(Some(response), Duration::from_millis(300));
        let (inbound, bucket) = mpsc::channel();
        let connection = establish_connection(
            "Cyberpunk",
            InstanceRegistry::bundled().unwrap(),
            "silvy",
            address,
            Duration::from_millis(100),
            inbound,
        )
        .unwrap();

        assert_eq!(
            bucket
                .recv_timeout(Duration::from_millis(200))
                .unwrap()
                .correlation_id(),
            Some("712345678900")
        );
        assert_eq!(
            bucket
                .recv_timeout(Duration::from_millis(200))
                .unwrap()
                .correlation_id(),
            Some("712345678900")
        );
        connection.close().unwrap();
    }
}
