use std::ops::Deref;

use time::OffsetDateTime;

use crate::background_jobs::JobLike;
use crate::database::custom_types::{Attempt, BackgroundJobId, BackgroundJobState, UniqueTaskKey};
use crate::database::{Database, DatabaseConnection};

pub struct CreateBackgroundJob<'a, JL>
where
    JL: JobLike,
{
    name: &'a str,
    queue_name: &'a str,

    unique_key: Option<&'a UniqueTaskKey>,
    task: &'a JL,

    attempt_run_at: OffsetDateTime,
}

impl<'a, JL: JobLike> CreateBackgroundJob<'a, JL> {
    pub fn now(
        name: &'a str,
        queue_name: &'a str,
        unique_key: Option<&'a UniqueTaskKey>,
        task: &'a JL,
    ) -> Self {
        Self::run_at(
            name,
            queue_name,
            unique_key,
            task,
            OffsetDateTime::now_utc(),
        )
    }

    pub fn run_at(
        name: &'a str,
        queue_name: &'a str,
        unique_key: Option<&'a UniqueTaskKey>,
        task: &'a JL,
        attempt_run_at: OffsetDateTime,
    ) -> Self {
        Self {
            name,
            queue_name,
            unique_key,
            task,
            attempt_run_at,
        }
    }

    pub async fn save(
        self,
        conn: &mut DatabaseConnection,
    ) -> Result<BackgroundJobId, BackgroundJobError> {
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
        .fetch_one(&mut *conn)
        .await
        .map_err(BackgroundJobError::SaveFailed)
    }
}

#[allow(dead_code)]
#[derive(sqlx::FromRow)]
pub struct BackgroundJob {
    id: BackgroundJobId,

    name: String,
    queue_name: String,

    unique_key: Option<UniqueTaskKey>,
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
