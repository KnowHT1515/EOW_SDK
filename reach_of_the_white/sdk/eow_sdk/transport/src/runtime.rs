//! Process-wide multi-instance TCP client runtime.

use std::collections::HashMap;
use std::net::{Shutdown, SocketAddrV4, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use crate::context::{InboundSink, context};
use crate::endpoint::InstanceRegistry;
use crate::envelope::SdkEnvelope;
use crate::error::{ConnectError, DisconnectError, EnvelopeError};
use crate::handshake::{HandshakeRequest, HandshakeResponse};
use crate::io::{FramedReader, FramedWriter};
use crate::limits::HANDSHAKE_SCHEMA;

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
    fn enqueue(&self, body: Vec<u8>) -> bool {
        self.alive.load(Ordering::Acquire) && self.sender.send(WriterCommand::Body(body)).is_ok()
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

    fn enqueue(&self, target: &str, body: Vec<u8>) -> bool {
        let connections = self
            .connections
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        connections
            .get(target)
            .is_some_and(|connection| connection.enqueue(body))
    }
}

pub fn connect(instance: &str) -> Result<(), ConnectError> {
    let context = context().ok_or(ConnectError::NotInitialized)?;
    let endpoint = context.registry.resolve(instance)?;
    context.runtime.connect(
        &context.sdk_id,
        &context.registry,
        instance,
        endpoint.address,
    )
}

#[doc(hidden)]
pub fn connect_at(instance: &str, address: SocketAddrV4) -> Result<(), ConnectError> {
    let context = context().ok_or(ConnectError::NotInitialized)?;
    let endpoint = context
        .registry
        .resolve_with_override(instance, Some(address))?;
    context.runtime.connect(
        &context.sdk_id,
        &context.registry,
        instance,
        endpoint.address,
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
    if context
        .runtime
        .enqueue(&target, sdk_envelope_json.into_bytes())
    {
        SendResult::accepted()
    } else {
        SendResult::rejected(Vec::new())
    }
}

fn establish_connection(
    sdk_id: &str,
    registry: InstanceRegistry,
    instance: &str,
    address: SocketAddrV4,
    inbound: InboundSink,
) -> Result<ConnectionHandle, ConnectError> {
    let mut stream = TcpStream::connect(address).map_err(ConnectError::Io)?;
    stream.set_nodelay(true).map_err(ConnectError::Io)?;

    {
        let mut writer = FramedWriter::new(&mut stream);
        writer.write_frame(&HandshakeRequest::new(sdk_id).to_body())?;
        writer.flush()?;
    }

    let response_body = {
        let mut reader = FramedReader::new(&mut stream);
        reader.read_frame()?.ok_or(ConnectError::HandshakeClosed)?
    };
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

    let read_stream = stream.try_clone().map_err(ConnectError::Io)?;
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
            let mut framed = FramedReader::new(read_stream);
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
