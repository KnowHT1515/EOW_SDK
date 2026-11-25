use std::{sync::Arc, time::Duration};

use serde::Serialize;
use serde_json::Value;
use tokio::sync::Mutex;

use crate::transport::SdkError;
use crate::transport::envelope::{Envelope, EnvelopeType};
use crate::transport::envelope_data::{ContextData, DemandData, RegisterData, ResultData, UnregisterData, UpdateData};
use crate::transport::sdk_transport::SdkTransport;
use crate::transport::tcp_connection::TcpConnection;

struct CloseOnDrop<'a>(Option<&'a TcpConnection>);

impl Drop for CloseOnDrop<'_> {

    fn drop(&mut self) {
        if let Some(connection) = self.0 {
            connection.close();
        }
    }

}

pub struct EowSdk {
    transport: SdkTransport,
    lifecycle: Mutex<()>,
}

impl EowSdk {

    pub fn new(sdk_id: impl Into<String>, port: u16) -> Self {
        Self::with_timeout(sdk_id, port, Duration::from_secs(30))
    }

    pub fn with_timeout(sdk_id: impl Into<String>, port: u16, timeout: Duration) -> Self {
        Self { transport: SdkTransport::new(sdk_id.into(), port, timeout), lifecycle: Mutex::new(()) }
    }

    pub fn sdk_id(&self) -> &str {
        self.transport.sdk_id()
    }

    pub async fn startup(&self) -> Result<Envelope, SdkError> {
        let _lifecycle = self.lifecycle.lock().await;
        let connection: Arc<TcpConnection> = self.transport.connect().await?;
        let mut close = CloseOnDrop(Some(&connection));
        let result: Result<(Envelope, bool), SdkError> = self.request_on(&connection, EnvelopeType::Startup, None).await.and_then(|response: Envelope| {
            let accepted: bool = match response.payload.payload_type.as_str() {
                "ack" => response.ack_ok().ok_or(SdkError::Protocol("startup ACK requires a boolean ok field"))?,
                "startup" => true,
                _ => return Err(SdkError::Protocol("unexpected startup response")),
            };
            Ok((response, accepted))
        });

        if let Ok((_, true)) = &result {
            self.transport.mark_ready()?;
            close.0 = None;
        }
        else {
            drop(close);
            self.transport.finish_close(&connection).await;
        }
        result.map(|(response, _): (Envelope, bool)| response)
    }

    pub async fn shutdown(&self) -> Result<Envelope, SdkError> {
        let _lifecycle = self.lifecycle.lock().await;
        let connection: Arc<TcpConnection> = self.transport.connection()?;
        let close = CloseOnDrop(Some(&connection));
        let result: Result<Envelope, SdkError> = self.request_on(&connection, EnvelopeType::Shutdown, None).await;
        drop(close);
        self.transport.finish_close(&connection).await;
        result
    }

    pub async fn context(&self, content: impl Into<String>, silent: bool) -> Result<Envelope, SdkError> {
        let data: ContextData = ContextData { content: content.into(), silent };
        self.request(EnvelopeType::Context, Some(serde_json::to_value(data)?)).await
    }

    pub async fn state_snapshot<T: Serialize + ?Sized>(&self, data: &T) -> Result<(), SdkError> {
        let connection: Arc<TcpConnection> = self.transport.connection()?;
        let data: Result<Value, SdkError> = serde_json::to_value(data).map_err(SdkError::from).and_then(|value: Value| {
            if value.is_object() {
                Ok(value)
            }
            else {
                Err(SdkError::InvalidData("Snapshot data must be a JSON object".into()))
            }
        });
        connection.send_snapshot(|revision: i32| self.transport.envelope_build(EnvelopeType::StateSnapshot, Some(data?), None, Some(revision))).await
    }

    pub async fn action_register(&self, data: &RegisterData) -> Result<Envelope, SdkError> {
        self.request(EnvelopeType::ActionRegister, Some(serde_json::to_value(data)?)).await
    }

    pub async fn action_unregister(&self, data: &UnregisterData) -> Result<Envelope, SdkError> {
        self.request(EnvelopeType::ActionUnregister, Some(serde_json::to_value(data)?)).await
    }

    pub async fn action_update(&self, data: &UpdateData) -> Result<Envelope, SdkError> {
        self.request(EnvelopeType::ActionUpdate, Some(serde_json::to_value(data)?)).await
    }

    pub async fn action_clear(&self) -> Result<Envelope, SdkError> {
        self.request(EnvelopeType::ActionClear, None).await
    }

    pub async fn action_demand(&self, data: &DemandData) -> Result<(Envelope, Option<Envelope>), SdkError> {
        let connection: Arc<TcpConnection> = self.transport.connection()?;
        let envelope: Envelope = self.transport.envelope_build(EnvelopeType::ActionDemand, Some(serde_json::to_value(data)?), None, None)?;
        connection.request(envelope, Some(&data.id)).await
    }

    pub async fn action_result(&self, correlation_id: &str, data: &ResultData) -> Result<(), SdkError> {
        let connection: Arc<TcpConnection> = self.transport.connection()?;
        let envelope: Envelope = self.transport.envelope_build(EnvelopeType::ActionResult, Some(serde_json::to_value(data)?), Some(correlation_id), None)?;
        connection.envelope_send(envelope).await
    }

    pub async fn next_action(&self) -> Result<Envelope, SdkError> {
        self.transport.connection()?.next_action().await
    }

    async fn request(&self, envelope_type: EnvelopeType, data: Option<Value>) -> Result<Envelope, SdkError> {
        let connection: Arc<TcpConnection> = self.transport.connection()?;
        self.request_on(&connection, envelope_type, data).await
    }

    async fn request_on(&self, connection: &TcpConnection, envelope_type: EnvelopeType, data: Option<Value>) -> Result<Envelope, SdkError> {
        let envelope: Envelope = self.transport.envelope_build(envelope_type, data, None, None)?;
        let (response, _): (Envelope, Option<Envelope>) = connection.request(envelope, None).await?;
        Ok(response)
    }

}
