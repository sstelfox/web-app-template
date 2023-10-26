use time::OffsetDateTime;

use crate::database::custom_types::{
    Attempt, BackgroundJobId, BackgroundRunId, BackgroundRunState,
};

#[derive(sqlx::FromRow)]
pub struct BackgroundRun {
    pub id: BackgroundRunId,

    attempt: Attempt,
    background_job_id: BackgroundJobId,
    state: BackgroundRunState,

    output: Option<serde_json::Value>,

    started_at: OffsetDateTime,
    finished_at: Option<OffsetDateTime>,
}