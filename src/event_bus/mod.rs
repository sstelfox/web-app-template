use bincode::Options;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

#[derive(Clone)]
pub struct EventBus {
    bus: broadcast::Sender<(SystemEvent, Vec<u8>)>,
}

impl EventBus {
    pub fn new() -> Self {
        let (bus, _) = broadcast::channel(1_024);
        Self { bus }
    }

    pub fn send(&self, event: SystemEvent, payload: &impl Serialize) -> Result<usize, EventBusError> {
        let bin_code_config = bincode::DefaultOptions::new();

        let bytes = bin_code_config.serialize(payload)
            .map_err(EventBusError::Serialization)?;

        self.bus.send((event, bytes))
            .map_err(EventBusError::SendFailed)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<(SystemEvent, Vec<u8>)> {
        self.bus.subscribe()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum EventBusError {
    #[error("failed to send message to the event bus: {0}")]
    SendFailed(broadcast::error::SendError<(SystemEvent, Vec<u8>)>),

    #[error("unable to serialize event payload: {0}")]
    Serialization(bincode::Error),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SystemEvent {
    TestEvent,
    UserRegistration,
}

use crate::database::custom_types::SessionId;

#[derive(Deserialize, Serialize)]
pub struct TestEvent {
    pub session_id: SessionId,
}

use crate::database::custom_types::UserId;

#[derive(Deserialize, Serialize)]
pub struct UserRegistration {
    pub id: UserId,
}
