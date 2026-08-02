use std::collections::HashMap;
use std::net::{Shutdown, SocketAddrV4, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::config::ServerConfig;
use crate::error::{ProtocolError, ServerError};
use crate::frame::{read_frame_interruptible, read_frame_until, write_frame};
use crate::message::NeuroBatch;
use crate::protocol::{
    BusinessMessage, EnvelopeKind, SdkEnvelope, ack, handshake_response, parse_business,
    parse_handshake,
};
use crate::translate::{ActionCache, Translation, translate};

type ActiveSessions = Arc<Mutex<HashMap<String, TcpStream>>>;

pub struct E2naServer {
    local_address: SocketAddrV4,
    running: Arc<AtomicBool>,
    sessions: ActiveSessions,
    accept_worker: Option<JoinHandle<Result<(), ServerError>>>,
}

impl E2naServer {
    pub fn bind(
        config: ServerConfig,
        neuro_queue: SyncSender<NeuroBatch>,
    ) -> Result<Self, ServerError> {
        let listener = TcpListener::bind(config.address)?;
        listener.set_nonblocking(true)?;
        let local_address = match listener.local_addr()? {
            std::net::SocketAddr::V4(address) => address,
            std::net::SocketAddr::V6(_) => unreachable!("E2NA binds an IPv4 address"),
        };
        let running = Arc::new(AtomicBool::new(true));
        let sessions = Arc::new(Mutex::new(HashMap::new()));
        let worker_running = Arc::clone(&running);
        let worker_sessions = Arc::clone(&sessions);
        let accept_worker = thread::Builder::new()
            .name("e2na-accept".into())
            .spawn(move || {
                accept_loop(
                    listener,
                    config,
                    neuro_queue,
                    worker_running,
                    worker_sessions,
                )
            })
            .map_err(ServerError::Spawn)?;

        Ok(Self {
            local_address,
            running,
            sessions,
            accept_worker: Some(accept_worker),
        })
    }

    pub fn local_address(&self) -> SocketAddrV4 {
        self.local_address
    }

    pub fn active_sdk_ids(&self) -> Vec<String> {
        let mut ids = self
            .sessions
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        ids.sort();
        ids
    }

    pub fn stop(mut self) -> Result<(), ServerError> {
        self.stop_inner()
    }

    fn stop_inner(&mut self) -> Result<(), ServerError> {
        self.running.store(false, Ordering::Release);
        {
            let sessions = self
                .sessions
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            for stream in sessions.values() {
                let _ = stream.set_read_timeout(Some(Duration::from_millis(10)));
                let _ = stream.shutdown(Shutdown::Both);
            }
        }
        match self.accept_worker.take() {
            Some(worker) => worker.join().map_err(|_| ServerError::WorkerPanicked)?,
            None => Ok(()),
        }
    }
}

impl Drop for E2naServer {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Release);
        let sessions = self
            .sessions
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        for stream in sessions.values() {
            let _ = stream.set_read_timeout(Some(Duration::from_millis(10)));
            let _ = stream.shutdown(Shutdown::Both);
        }
        let _ = self.accept_worker.take();
    }
}

fn accept_loop(
    listener: TcpListener,
    config: ServerConfig,
    neuro_queue: SyncSender<NeuroBatch>,
    running: Arc<AtomicBool>,
    sessions: ActiveSessions,
) -> Result<(), ServerError> {
    let mut workers = Vec::new();
    while running.load(Ordering::Acquire) {
        match listener.accept() {
            Ok((stream, _)) => {
                let connection_running = Arc::clone(&running);
                let connection_sessions = Arc::clone(&sessions);
                let connection_queue = neuro_queue.clone();
                let worker = thread::Builder::new()
                    .name("e2na-session".into())
                    .spawn(move || {
                        let _ = handle_connection(
                            stream,
                            config,
                            connection_queue,
                            connection_running,
                            connection_sessions,
                        );
                    })
                    .map_err(ServerError::Spawn)?;
                workers.push(worker);
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(5));
            }
            Err(_error) if !running.load(Ordering::Acquire) => break,
            Err(error) => return Err(ServerError::Io(error)),
        }
    }

    {
        let sessions = sessions
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        for stream in sessions.values() {
            let _ = stream.set_read_timeout(Some(Duration::from_millis(10)));
            let _ = stream.shutdown(Shutdown::Both);
        }
    }
    let mut panicked = false;
    for worker in workers {
        panicked |= worker.join().is_err();
    }
    if panicked {
        Err(ServerError::WorkerPanicked)
    } else {
        Ok(())
    }
}

fn handle_connection(
    mut stream: TcpStream,
    config: ServerConfig,
    neuro_queue: SyncSender<NeuroBatch>,
    running: Arc<AtomicBool>,
    sessions: ActiveSessions,
) -> Result<(), ProtocolError> {
    stream.set_nonblocking(false)?;
    stream.set_nodelay(true)?;
    let deadline = Instant::now()
        .checked_add(config.handshake_timeout)
        .ok_or(ProtocolError::Invalid("handshake_timeout"))?;
    stream.set_write_timeout(Some(config.handshake_timeout))?;
    let handshake = match read_frame_until(&mut stream, deadline)? {
        Some(body) => body,
        None => return Ok(()),
    };
    stream.set_write_timeout(Some(remaining_until(deadline)?))?;
    let sdk_id = match parse_handshake(&handshake) {
        Ok(sdk_id) => sdk_id,
        Err(_) => {
            write_frame(&mut stream, &handshake_response(false))?;
            return Ok(());
        }
    };
    if !running.load(Ordering::Acquire) {
        write_frame(&mut stream, &handshake_response(false))?;
        return Ok(());
    }

    let control = stream.try_clone()?;
    let reserved = {
        let mut sessions = sessions
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if sessions.contains_key(&sdk_id) {
            false
        } else {
            sessions.insert(sdk_id.clone(), control);
            true
        }
    };
    if !reserved {
        write_frame(&mut stream, &handshake_response(false))?;
        return Ok(());
    }

    let result = (|| {
        write_frame(&mut stream, &handshake_response(true))?;
        stream.set_read_timeout(Some(Duration::from_millis(100)))?;
        stream.set_write_timeout(None)?;
        run_session(&mut stream, &sdk_id, &neuro_queue, &running)
    })();
    sessions
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(&sdk_id);
    result
}

fn remaining_until(deadline: Instant) -> Result<Duration, ProtocolError> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        Err(ProtocolError::Invalid("handshake_timeout"))
    } else {
        Ok(remaining)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SessionState {
    Handshaken,
    Started,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QueueOutcome {
    Accepted,
    Full,
    Closed,
}

fn run_session(
    stream: &mut TcpStream,
    sdk_id: &str,
    neuro_queue: &SyncSender<NeuroBatch>,
    running: &AtomicBool,
) -> Result<(), ProtocolError> {
    let mut state = SessionState::Handshaken;
    let mut actions = ActionCache::new();
    loop {
        let body = match read_frame_interruptible(stream, running)? {
            Some(body) => body,
            None => return Ok(()),
        };
        let envelope = match SdkEnvelope::parse(&body, sdk_id) {
            Ok(envelope) => envelope,
            Err(_) => return Ok(()),
        };
        let message = match parse_business(&envelope) {
            Ok(message) => message,
            Err(_) if envelope.kind == EnvelopeKind::Request => {
                write_frame(stream, &ack(sdk_id, &envelope.event_id, false))?;
                continue;
            }
            Err(_) => return Ok(()),
        };

        match envelope.kind {
            EnvelopeKind::Request => {
                let (ok, close) =
                    process_request(&message, sdk_id, neuro_queue, &mut state, &mut actions);
                write_frame(stream, &ack(sdk_id, &envelope.event_id, ok))?;
                if close {
                    return Ok(());
                }
            }
            EnvelopeKind::Response => {
                if state != SessionState::Started
                    || !matches!(message, BusinessMessage::ActionResult { .. })
                {
                    return Ok(());
                }
                let translation = translate(&message, sdk_id, &actions);
                if enqueue(neuro_queue, sdk_id, translation.messages) != QueueOutcome::Accepted {
                    return Ok(());
                }
            }
            EnvelopeKind::Event => return Ok(()),
        }
    }
}

fn process_request(
    message: &BusinessMessage,
    sdk_id: &str,
    neuro_queue: &SyncSender<NeuroBatch>,
    state: &mut SessionState,
    actions: &mut ActionCache,
) -> (bool, bool) {
    match message {
        BusinessMessage::Startup if *state == SessionState::Handshaken => {
            let translation = translate(message, sdk_id, actions);
            match enqueue(neuro_queue, sdk_id, translation.messages) {
                QueueOutcome::Accepted => {
                    *state = SessionState::Started;
                    (true, false)
                }
                QueueOutcome::Full => (false, false),
                QueueOutcome::Closed => (false, true),
            }
        }
        BusinessMessage::Startup => (false, false),
        BusinessMessage::Shutdown if *state == SessionState::Started => (true, true),
        BusinessMessage::Shutdown => (false, false),
        _ if *state != SessionState::Started => (false, false),
        _ => {
            let Translation {
                messages,
                next_actions,
            } = translate(message, sdk_id, actions);
            match enqueue(neuro_queue, sdk_id, messages) {
                QueueOutcome::Accepted => {
                    if let Some(next_actions) = next_actions {
                        *actions = next_actions;
                    }
                    (true, false)
                }
                QueueOutcome::Full => (false, false),
                QueueOutcome::Closed => (false, true),
            }
        }
    }
}

fn enqueue(queue: &SyncSender<NeuroBatch>, sdk_id: &str, messages: Vec<String>) -> QueueOutcome {
    if messages.is_empty() {
        return QueueOutcome::Accepted;
    }
    match queue.try_send(NeuroBatch::new(sdk_id, messages)) {
        Ok(()) => QueueOutcome::Accepted,
        Err(TrySendError::Full(_)) => QueueOutcome::Full,
        Err(TrySendError::Disconnected(_)) => QueueOutcome::Closed,
    }
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, SocketAddrV4};
    use std::sync::mpsc::{self, Receiver};

    use serde_json::{Value, json};

    use super::*;
    use crate::frame::read_frame;

    fn test_server() -> (E2naServer, Receiver<NeuroBatch>) {
        test_server_with_capacity(16)
    }

    fn test_server_with_capacity(capacity: usize) -> (E2naServer, Receiver<NeuroBatch>) {
        let (sender, receiver) = mpsc::sync_channel(capacity);
        let server = E2naServer::bind(
            ServerConfig {
                address: SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0),
                ..ServerConfig::default()
            },
            sender,
        )
        .unwrap();
        (server, receiver)
    }

    fn connect(server: &E2naServer, sdk_id: &str) -> (TcpStream, Value) {
        let mut stream = TcpStream::connect(server.local_address()).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        write_frame(
            &mut stream,
            &serde_json::to_vec(&json!({
                "handshake": {"schema": "1.0", "id": sdk_id}
            }))
            .unwrap(),
        )
        .unwrap();
        let response = read_frame(&mut stream).unwrap().unwrap();
        (stream, serde_json::from_slice(&response).unwrap())
    }

    fn envelope(sdk_id: &str, message_type: &str, extra: Value) -> Vec<u8> {
        let mut payload = json!({"sdk_id": sdk_id, "type": message_type});
        if let (Some(payload), Some(extra)) = (payload.as_object_mut(), extra.as_object()) {
            payload.extend(extra.clone());
        }
        serde_json::to_vec(&json!({
            "envelope_schema": "1.0",
            "envelope_kind": "request",
            "source": "eow_sdk",
            "target": "neuro",
            "event_id": "712345678901",
            "correlation_id": null,
            "payload": payload,
        }))
        .unwrap()
    }

    fn send_and_ack(stream: &mut TcpStream, body: &[u8]) -> Value {
        write_frame(stream, body).unwrap();
        let response = read_frame(stream).unwrap().unwrap();
        serde_json::from_slice(&response).unwrap()
    }

    fn startup(stream: &mut TcpStream, receiver: &Receiver<NeuroBatch>) {
        let ack = send_and_ack(stream, &envelope("game", "startup", json!({})));
        assert_eq!(ack["payload"]["data"]["ok"], true);
        let batch = receiver.recv_timeout(Duration::from_secs(1)).unwrap();
        assert_eq!(batch.sdk_id, "game");
        let startup: Value = serde_json::from_str(&batch.messages[0]).unwrap();
        assert_eq!(startup["command"], "startup");
    }

    #[test]
    fn lifecycle_requires_startup_and_shutdown_closes_the_connection() {
        let (server, receiver) = test_server();
        let (mut stream, response) = connect(&server, "game");
        assert_eq!(response["handshake"]["result"], "accept");

        let context = envelope(
            "game",
            "context",
            json!({"data": {"content": "before", "silent": true}}),
        );
        assert_eq!(
            send_and_ack(&mut stream, &context)["payload"]["data"]["ok"],
            false
        );
        startup(&mut stream, &receiver);
        assert_eq!(
            send_and_ack(&mut stream, &envelope("game", "startup", json!({})))["payload"]["data"]["ok"],
            false
        );
        assert_eq!(
            send_and_ack(&mut stream, &envelope("game", "shutdown", json!({})))["payload"]["data"]
                ["ok"],
            true
        );
        assert!(read_frame(&mut stream).unwrap().is_none());
        server.stop().unwrap();
    }

    #[test]
    fn update_is_enqueued_as_one_ordered_diff_batch() {
        let (server, receiver) = test_server();
        let (mut stream, _) = connect(&server, "game");
        startup(&mut stream, &receiver);

        let first = envelope(
            "game",
            "action",
            json!({
                "kind": "update",
                "data": [
                    {"action_id": "change", "description": "old"},
                    {"action_id": "remove", "description": "old"}
                ]
            }),
        );
        assert_eq!(
            send_and_ack(&mut stream, &first)["payload"]["data"]["ok"],
            true
        );
        receiver.recv_timeout(Duration::from_secs(1)).unwrap();

        let second = envelope(
            "game",
            "action",
            json!({
                "kind": "update",
                "data": [
                    {"action_id": "add", "description": "new"},
                    {"action_id": "change", "description": "new"}
                ]
            }),
        );
        assert_eq!(
            send_and_ack(&mut stream, &second)["payload"]["data"]["ok"],
            true
        );
        let batch = receiver.recv_timeout(Duration::from_secs(1)).unwrap();
        assert_eq!(batch.messages.len(), 2);
        let unregister: Value = serde_json::from_str(&batch.messages[0]).unwrap();
        let register: Value = serde_json::from_str(&batch.messages[1]).unwrap();
        assert_eq!(unregister["command"], "actions/unregister");
        assert_eq!(register["command"], "actions/register");
        server.stop().unwrap();
    }

    #[test]
    fn full_queue_returns_failure_and_rolls_back_action_cache() {
        let (server, receiver) = test_server_with_capacity(1);
        let (mut stream, _) = connect(&server, "game");
        startup(&mut stream, &receiver);

        let first = envelope(
            "game",
            "action",
            json!({
                "kind": "update",
                "data": [{"action_id": "a", "description": "first"}]
            }),
        );
        assert_eq!(
            send_and_ack(&mut stream, &first)["payload"]["data"]["ok"],
            true
        );
        let second = envelope(
            "game",
            "action",
            json!({
                "kind": "update",
                "data": [{"action_id": "b", "description": "second"}]
            }),
        );
        assert_eq!(
            send_and_ack(&mut stream, &second)["payload"]["data"]["ok"],
            false
        );
        receiver.recv_timeout(Duration::from_secs(1)).unwrap();

        let clear = envelope("game", "action", json!({"kind": "clear"}));
        assert_eq!(
            send_and_ack(&mut stream, &clear)["payload"]["data"]["ok"],
            true
        );
        let batch = receiver.recv_timeout(Duration::from_secs(1)).unwrap();
        let unregister: Value = serde_json::from_str(&batch.messages[0]).unwrap();
        assert_eq!(unregister["data"]["action_names"], json!(["a"]));
        server.stop().unwrap();
    }

    #[test]
    fn response_with_unknown_correlation_is_forwarded_without_ack() {
        let (server, receiver) = test_server();
        let (mut stream, _) = connect(&server, "game");
        startup(&mut stream, &receiver);
        let response = serde_json::to_vec(&json!({
            "envelope_schema": "1.0",
            "envelope_kind": "response",
            "source": "eow_sdk",
            "target": "neuro",
            "event_id": "712345678902",
            "correlation_id": "799999999999",
            "payload": {
                "sdk_id": "game",
                "type": "action",
                "kind": "result",
                "data": {"id": "call-1", "ok": true, "result": "done"}
            }
        }))
        .unwrap();
        write_frame(&mut stream, &response).unwrap();
        let batch = receiver.recv_timeout(Duration::from_secs(1)).unwrap();
        let result: Value = serde_json::from_str(&batch.messages[0]).unwrap();
        assert_eq!(result["command"], "action/result");
        assert_eq!(result["data"]["message"], "done");

        let context = envelope(
            "game",
            "context",
            json!({"data": {"content": "still connected", "silent": true}}),
        );
        assert_eq!(
            send_and_ack(&mut stream, &context)["payload"]["data"]["ok"],
            true
        );
        receiver.recv_timeout(Duration::from_secs(1)).unwrap();
        server.stop().unwrap();
    }

    #[test]
    fn different_sdk_ids_can_remain_connected_concurrently() {
        let (server, _receiver) = test_server();
        let (_first, first_response) = connect(&server, "game-a");
        let (_second, second_response) = connect(&server, "game-b");
        assert_eq!(first_response["handshake"]["result"], "accept");
        assert_eq!(second_response["handshake"]["result"], "accept");
        assert_eq!(server.active_sdk_ids(), ["game-a", "game-b"]);
        server.stop().unwrap();
    }

    #[test]
    fn disconnected_neuro_queue_fails_startup_ack_and_closes_session() {
        let (sender, receiver) = mpsc::sync_channel(1);
        drop(receiver);
        let server = E2naServer::bind(
            ServerConfig {
                address: SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0),
                ..ServerConfig::default()
            },
            sender,
        )
        .unwrap();
        let (mut stream, _) = connect(&server, "game");
        let startup = envelope("game", "startup", json!({}));
        assert_eq!(
            send_and_ack(&mut stream, &startup)["payload"]["data"]["ok"],
            false
        );
        assert!(read_frame(&mut stream).unwrap().is_none());
        server.stop().unwrap();
    }

    #[test]
    fn invalid_business_payload_fails_ack_without_closing_session() {
        let (server, receiver) = test_server();
        let (mut stream, _) = connect(&server, "game");
        startup(&mut stream, &receiver);
        let invalid = envelope(
            "game",
            "context",
            json!({"data": {"content": "missing silent"}}),
        );
        assert_eq!(
            send_and_ack(&mut stream, &invalid)["payload"]["data"]["ok"],
            false
        );
        let valid = envelope(
            "game",
            "context",
            json!({"data": {"content": "still connected", "silent": true}}),
        );
        assert_eq!(
            send_and_ack(&mut stream, &valid)["payload"]["data"]["ok"],
            true
        );
        let batch = receiver.recv_timeout(Duration::from_secs(1)).unwrap();
        let context: Value = serde_json::from_str(&batch.messages[0]).unwrap();
        assert_eq!(context["command"], "context");
        server.stop().unwrap();
    }

    #[test]
    fn duplicate_active_sdk_id_is_denied() {
        let (server, _receiver) = test_server();
        let (_first, accepted) = connect(&server, "game");
        assert_eq!(accepted["handshake"]["result"], "accept");
        let (_second, denied) = connect(&server, "game");
        assert_eq!(denied["handshake"]["result"], "deny");
        server.stop().unwrap();
    }
}
