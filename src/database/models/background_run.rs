use time::OffsetDateTime;

use crate::database::{
    custom_types::{Attempt, BackgroundJobId, BackgroundRunId, BackgroundRunState},
    DatabaseConnection,
};

pub struct CreateBackgroundRun<'a> {
    background_job_id: &'a BackgroundJobId,
}

impl<'a> CreateBackgroundRun<'a> {
    pub async fn save(
        self,
        conn: &mut DatabaseConnection,
    ) -> Result<BackgroundRunId, BackgroundRunError> {
        let attempt = Attempt::zero();
        let started_at = OffsetDateTime::now_utc();

        sqlx::query_scalar!(
            r#"INSERT INTO background_runs (background_job_id, attempt, state, started_at)
                   VALUES ($1, $2, $3, $4)
                   RETURNING id as 'id: BackgroundRunId';"#,
            self.background_job_id,
            attempt,
            BackgroundRunState::Running,
            started_at,
        )
        .fetch_one(&mut *conn)
        .await
        .map_err(BackgroundRunError::SaveFailed)
    }
}

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
pub enum BackgroundRunError {
    #[error("failed to save background run: {0}")]
    SaveFailed(sqlx::Error),
}
