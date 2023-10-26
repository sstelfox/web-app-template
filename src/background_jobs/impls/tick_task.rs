use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::background_jobs::{EventTaskContext, JobLike};
use crate::database::custom_types::UniqueTaskKey;
use crate::event_bus::{EventBusError, SystemEvent};

#[derive(Deserialize, Serialize)]
pub struct TickTask;

#[async_trait]
impl JobLike for TickTask {
    const JOB_NAME: &'static str = "tick_task";

    type Error = TickTaskError;
    type Context = EventTaskContext;

    async fn run(&self, ctx: Self::Context) -> Result<(), Self::Error> {
        ctx.event_bus()
            .send(SystemEvent::Tick, &TickMessage::now())
            .map_err(TickTaskError::SendFailed)?;

        Ok(())
    }

    /// We only ever want a single one of these enqueued at a time so this uses a fixed unique key
    /// for all instances.
    async fn unique_key(&self) -> Option<UniqueTaskKey> {
        Some(UniqueTaskKey::from("tick"))
    }
}

#[derive(Deserialize, Serialize)]
pub struct TickMessage {
    time: OffsetDateTime,
}

impl TickMessage {
    fn now() -> Self {
        Self {
            time: OffsetDateTime::now_utc(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TickTaskError {
    #[error("failed to send tick: {0}")]
    SendFailed(EventBusError),
}
