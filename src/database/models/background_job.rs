#![allow(dead_code)]

use std::ops::Deref;

use time::OffsetDateTime;

use crate::background_jobs::JobLike;
use crate::database::custom_types::{Attempt, BackgroundJobId, BackgroundJobState};
use crate::database::Database;

pub struct CreateBackgroundJob<'a, JL>
where
    JL: JobLike,
{
    pub name: &'a str,
    pub queue_name: &'a str,

    pub unique_key: Option<&'a str>,
    pub task: &'a JL,

    pub attempt_run_at: OffsetDateTime,
}

impl<'a, JL: JobLike> CreateBackgroundJob<'a, JL> {
    pub async fn save(self, database: &Database) -> Result<BackgroundJobId, BackgroundJobError> {
        let payload = serde_json::to_string(self.task)
            .map_err(BackgroundJobError::PayloadSerializationFailed)?;

        sqlx::query_scalar!(
            r#"INSERT INTO background_jobs (name, queue_name, unique_key, state,
                       current_attempt, maximum_attempts, payload, attempt_run_at)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                   RETURNING id as 'id: BackgroundJobId';"#,
            self.name,
            self.queue_name,
            self.unique_key,
            BackgroundJobState::Scheduled,
            0u8,
            JL::MAX_ATTEMPTS,
            payload,
            self.attempt_run_at,
        )
        .fetch_one(database.deref())
        .await
        .map_err(BackgroundJobError::SaveFailed)
    }
}

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

#[derive(Debug, thiserror::Error)]
pub enum BackgroundJobError {
    #[error("failed to serialize task payload: {0}")]
    PayloadSerializationFailed(serde_json::Error),

    #[error("failed to save background job: {0}")]
    SaveFailed(sqlx::Error),
}
