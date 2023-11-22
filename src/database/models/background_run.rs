use time::OffsetDateTime;

use crate::database::custom_types::{
    Attempt, BackgroundJobId, BackgroundRunId, BackgroundRunState,
};

#[allow(dead_code)]
#[derive(sqlx::FromRow)]
pub struct BackgroundRun {
    id: BackgroundRunId,

    attempt: Attempt,
    background_job_id: BackgroundJobId,
    state: BackgroundRunState,

    output: Option<serde_json::Value>,

    started_at: OffsetDateTime,
    finished_at: Option<OffsetDateTime>,
}

#[derive(Debug, thiserror::Error)]
pub enum BackgroundJobError {
    #[error("failed to save background run: {0}")]
    SaveFailed(sqlx::Error),
}
