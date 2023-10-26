#![allow(dead_code)]

use time::OffsetDateTime;

use crate::database::custom_types::{Attempt, BackgroundJobId, BackgroundJobState};

#[derive(sqlx::FromRow)]
pub struct BackgroundJob {
    id: BackgroundJobId,

    name: String,
    queue_name: String,

    unique_key: Option<String>,
    state: BackgroundJobState,

    current_attempt: Attempt,
    maximum_attempts: Attempt,

    payload: Option<serde_json::Value>,

    scheduled_at: OffsetDateTime,
    attempt_run_at: OffsetDateTime,
}

impl BackgroundJob {
    pub fn id(&self) -> BackgroundJobId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn payload(&self) -> Option<&serde_json::Value> {
        self.payload.as_ref()
    }
}
