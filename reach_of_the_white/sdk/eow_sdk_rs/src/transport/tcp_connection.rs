use std::{collections::{HashMap, hash_map::Entry}, sync::{Arc, Mutex}, time::Duration};

use serde_json::Value;
use tokio::{io::{AsyncRead, AsyncWrite}, net::TcpStream, sync::{Mutex as AsyncMutex, mpsc, oneshot, watch}, task::JoinHandle, time::{Instant, timeout_at}};

use crate::transport::SdkError;
use crate::transport::envelope::{Envelope, EnvelopeKind};
use crate::transport::tcp_framing;

type Reply = (Envelope, Option<Envelope>);

struct WriteFrame {
    body: Vec<u8>,
    deadline: Instant,
    completion: oneshot::Sender<Result<(), SdkError>>,
}

struct Pending {
    demand_id: Option<String>,
    ack: Option<Envelope>,
    completion: Option<oneshot::Sender<Result<Reply, SdkError>>>,
}

impl Pending {

    fn receive_response(&mut self, response: Envelope) {
        if self.demand_id.is_none() {
            self.complete(Ok((response, None)));
        }
        else if self.ack.is_none() && self.completion.is_some() {
            match response.ack_ok() {
                Some(true) => self.ack = Some(response),
                Some(false) => self.complete(Ok((response, None))),
                None => self.complete(Err(SdkError::Protocol("action demand requires an ACK with a boolean ok field"))),
            }
        }
    }

    fn complete(&mut self, result: Result<Reply, SdkError>) {
        if let Some(completion) = self.completion.take() {
            let _ = completion.send(result);
        }
    }

}

#[derive(Default)]
struct PendingRequests {
    responses: HashMap<String, Pending>,
    demand: Option<String>,
}

struct Waiting<'a> {
    shared: &'a ConnectionState,
    event_id: String,
    response: oneshot::Receiver<Result<Reply, SdkError>>,
}

impl Drop for Waiting<'_> {

    fn drop(&mut self) {
        let mut pending = self.shared.pending.lock().unwrap();
        pending.responses.remove(&self.event_id);
        if pending.demand.as_ref() == Some(&self.event_id) {
            pending.demand = None;
        }
    }

}

struct ConnectionState {
    pending: Mutex<PendingRequests>,
    closed: watch::Sender<Option<SdkError>>,
}

struct CloseOnWorkerExit(Arc<ConnectionState>);

impl Drop for CloseOnWorkerExit {

    fn drop(&mut self) {
        self.0.close(SdkError::Disconnected);
    }

}

impl ConnectionState {

    fn expect_response(&self, event_id: String, demand_id: Option<&str>) -> Result<Waiting<'_>, SdkError> {
        let mut pending = self.pending.lock().unwrap();
        if let Some(error) = self.closed.borrow().clone() {
            return Err(error);
        }
        if demand_id.is_some() && pending.demand.is_some() {
            return Err(SdkError::DemandPending);
        }
        let Entry::Vacant(entry) = pending.responses.entry(event_id.clone()) else {
            return Err(SdkError::EventIdExhausted);
        };
        let (completion, response) = oneshot::channel();
        entry.insert(Pending { demand_id: demand_id.map(str::to_owned), ack: None, completion: Some(completion) });
        if demand_id.is_some() {
            pending.demand = Some(event_id.clone());
        }
        Ok(Waiting { shared: self, event_id, response })
    }

    fn dispatch(&self, envelope: Envelope, actions: &mpsc::UnboundedSender<Envelope>) {
        let PendingRequests { responses, demand } = &mut *self.pending.lock().unwrap();
        match envelope.envelope_kind {
            EnvelopeKind::Response => {
                if let Some(waiter) = envelope.correlation_id.as_ref().and_then(|id: &String| responses.get_mut(id)) {
                    waiter.receive_response(envelope);
                }
            },
            EnvelopeKind::Request if envelope.payload.payload_type == "action" => {
                let demand_id: Option<&str> = envelope.payload.data.as_ref().and_then(|data: &Value| data.get("demand_id")).and_then(Value::as_str);
                if let Some(waiter) = demand.as_ref().and_then(|id: &String| responses.get_mut(id))
                    && waiter.demand_id.as_deref() == demand_id
                    && let Some(ack) = waiter.ack.take() {
                    waiter.complete(Ok((ack, Some(envelope))));
                }
                else {
                    let _ = actions.send(envelope);
                }
            },
            _ => {},
        }
    }

    fn close(&self, error: SdkError) {
        let mut pending = self.pending.lock().unwrap();
        if self.closed.borrow().is_none() {
            self.closed.send_replace(Some(error));
            pending.responses.clear();
            pending.demand = None;
        }
    }

    async fn wait_closed(&self) -> SdkError {
        let mut closed = self.closed.subscribe();
        let reason = closed.wait_for(Option::is_some).await.expect("connection state owns the close sender");
        reason.clone().expect("close notification has a reason")
    }

}

pub struct TcpConnection {
    timeout: Duration,
    shared: Arc<ConnectionState>,
    writer: mpsc::UnboundedSender<WriteFrame>,
    actions: AsyncMutex<mpsc::UnboundedReceiver<Envelope>>,
    worker: AsyncMutex<Option<JoinHandle<()>>>,
    next_revision: Mutex<i64>,
}

impl TcpConnection {

    pub fn new(mut stream: TcpStream, timeout: Duration) -> Self {
        let (writer, writes) = mpsc::unbounded_channel();
        let (incoming, actions) = mpsc::unbounded_channel();
        let (closed, _) = watch::channel(None);
        let shared: Arc<ConnectionState> = Arc::new(ConnectionState { pending: Mutex::new(PendingRequests::default()), closed });
        let on_exit = CloseOnWorkerExit(Arc::clone(&shared));
        let worker: JoinHandle<()> = tokio::spawn(async move {
            let on_exit = on_exit;
            let state: &ConnectionState = &on_exit.0;
            let error: SdkError = {
                let (mut reader, mut writer) = stream.split();
                tokio::select! {
                    biased;
                    error = state.wait_closed() => error,
                    result = Self::receive(&mut reader, state, incoming) => result.err().unwrap_or(SdkError::Disconnected),
                    result = Self::transmit(&mut writer, state, writes) => result.err().unwrap_or(SdkError::Disconnected),
                }
            };
            drop(stream);
            state.close(error);
        });
        Self { timeout, shared, writer, actions: AsyncMutex::new(actions), worker: AsyncMutex::new(Some(worker)), next_revision: Mutex::new(0) }
    }

    pub fn ensure_open(&self) -> Result<(), SdkError> {
        self.shared.closed.borrow().clone().map_or(Ok(()), Err)
    }

    pub async fn request(&self, envelope: Envelope, demand_id: Option<&str>) -> Result<Reply, SdkError> {
        let deadline: Instant = self.deadline()?;
        let mut waiting: Waiting<'_> = self.shared.expect_response(envelope.event_id.clone(), demand_id)?;
        let mut sent = self.enqueue(envelope, deadline)?;
        self.wait_completion(&mut sent, deadline).await?;
        self.wait_completion(&mut waiting.response, deadline).await
    }

    pub async fn envelope_send(&self, envelope: Envelope) -> Result<(), SdkError> {
        let deadline: Instant = self.deadline()?;
        let mut sent = self.enqueue(envelope, deadline)?;
        self.wait_completion(&mut sent, deadline).await
    }

    pub async fn send_snapshot(&self, build: impl FnOnce(i32) -> Result<Envelope, SdkError>) -> Result<(), SdkError> {
        let deadline: Instant = self.deadline()?;
        let mut sent = {
            let mut next_revision = self.next_revision.lock().unwrap();
            let revision: i32 = i32::try_from(*next_revision).map_err(|_| SdkError::InvalidData("Snapshot revision is exhausted".into()))?;
            *next_revision += 1;
            self.enqueue(build(revision)?, deadline)?
        };
        self.wait_completion(&mut sent, deadline).await
    }

    pub async fn next_action(&self) -> Result<Envelope, SdkError> {
        tokio::select! {
            biased;
            error = self.shared.wait_closed() => Err(error),
            action = async { self.actions.lock().await.recv().await } => action.ok_or_else(|| self.close_error()),
        }
    }

    pub fn close(&self) {
        self.shared.close(SdkError::Disconnected);
    }

    pub async fn wait_closed(&self) {
        let mut worker = self.worker.lock().await;
        if let Some(task) = worker.as_mut() {
            let _ = task.await;
            *worker = None;
        }
    }

    fn deadline(&self) -> Result<Instant, SdkError> {
        Instant::now().checked_add(self.timeout).ok_or_else(|| SdkError::InvalidData("SDK timeout is out of range".into()))
    }

    fn enqueue(&self, envelope: Envelope, deadline: Instant) -> Result<oneshot::Receiver<Result<(), SdkError>>, SdkError> {
        self.ensure_open()?;
        let body: Vec<u8> = serde_json::to_vec(&envelope)?;
        tcp_framing::validate_length(body.len())?;
        let (completion, sent) = oneshot::channel();
        self.writer.send(WriteFrame { body, deadline, completion }).map_err(|_| self.close_error())?;
        Ok(sent)
    }

    async fn wait_completion<T>(&self, completion: &mut oneshot::Receiver<Result<T, SdkError>>, deadline: Instant) -> Result<T, SdkError> {
        timeout_at(deadline, completion).await.map_err(|_| SdkError::Timeout)?.map_err(|_| self.close_error())?
    }

    fn close_error(&self) -> SdkError {
        self.shared.closed.borrow().clone().unwrap_or(SdkError::Disconnected)
    }

    async fn receive(reader: &mut (impl AsyncRead + Unpin), state: &ConnectionState, actions: mpsc::UnboundedSender<Envelope>) -> Result<(), SdkError> {
        while let Some(body) = tcp_framing::read_frame(reader).await? {
            let envelope: Envelope = serde_json::from_slice(&body)?;
            state.dispatch(envelope, &actions);
        }
        Ok(())
    }

    async fn transmit(writer: &mut (impl AsyncWrite + Unpin), state: &ConnectionState, mut writes: mpsc::UnboundedReceiver<WriteFrame>) -> Result<(), SdkError> {
        while let Some(frame) = writes.recv().await {
            if frame.completion.is_closed() {
                continue;
            }
            if Instant::now() >= frame.deadline {
                let _ = frame.completion.send(Err(SdkError::Timeout));
                continue;
            }
            match timeout_at(frame.deadline, tcp_framing::write_frame(writer, &frame.body)).await.unwrap_or(Err(SdkError::Timeout)) {
                Ok(()) => {
                    let _ = frame.completion.send(Ok(()));
                },
                Err(error) => {
                    state.close(error.clone());
                    let _ = frame.completion.send(Err(error.clone()));
                    return Err(error);
                },
            }
        }
        Ok(())
    }

}

impl Drop for TcpConnection {

    fn drop(&mut self) {
        self.close();
    }

}
