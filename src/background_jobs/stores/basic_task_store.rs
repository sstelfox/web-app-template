use async_trait::async_trait;
use sqlx::SqlitePool;

use crate::background_jobs::stores::{JobStore, JobStoreError};
use crate::background_jobs::JobLike;
use crate::database::custom_types::{BackgroundJobId, BackgroundJobState, BackgroundRunId};
use crate::database::models::{BackgroundJob, BackgroundJobError, CreateBackgroundJob};
use crate::database::Database;

#[derive(Clone)]
pub struct BasicTaskContext {
    database: Database,
}

impl BasicTaskContext {
    pub fn new(database: Database) -> Self {
        Self { database }
    }
}

#[derive(Clone)]
pub struct BasicTaskStore {
    context: BasicTaskContext,
}

impl BasicTaskStore {
    pub fn context(&self) -> BasicTaskContext {
        self.context.clone()
    }

    pub fn new(context: BasicTaskContext) -> Self {
        Self { context }
    }
}

#[async_trait]
impl JobStore for BasicTaskStore {
    type Connection = SqlitePool;

    //async fn cancel(&self, id: BackgroundJobId) -> Result<(), JobStoreError> {
    //    self.update_state(id, BackgroundJobState::Cancelled).await
    //}

    async fn enqueue<JL: JobLike>(
        pool: &mut Self::Connection,
        job: JL,
    ) -> Result<Option<(BackgroundJobId, BackgroundRunId)>, JobStoreError>
    where
        Self: Sized,
    {
        let mut conn = pool.begin().await.map_err(BasicStoreError::Connection)?;
        let unique_key = job.unique_key().await;

        if let Some(key) = &unique_key {
            if key.is_active(&mut conn).await? {
                return Ok(None);
            }
        }

        let _background_job_id =
            CreateBackgroundJob::now(JL::JOB_NAME, JL::QUEUE_NAME, unique_key.as_ref(), &job)
                .save(&mut conn)
                .await
                .map_err(BasicStoreError::BackgroundJob)?;

        conn.commit().await.map_err(BasicStoreError::Transaction)?;

        todo!()
    }

    async fn next(
        &self,
        _queue_name: &str,
        _job_names: &[&str],
    ) -> Result<Option<BackgroundJob>, JobStoreError> {
        todo!()
    }

    async fn retry(&self, _id: BackgroundJobId) -> Result<Option<BackgroundRunId>, JobStoreError> {
        todo!()
    }

    async fn update_state(
        &self,
        _id: BackgroundJobId,
        _new_state: BackgroundJobState,
    ) -> Result<(), JobStoreError> {
        todo!()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BasicStoreError {
    #[error("background job query failed: {0}")]
    BackgroundJob(BackgroundJobError),

    #[error("failed to acquire connection from pool: {0}")]
    Connection(sqlx::Error),

    #[error("an error occurred with a transaction operation: {0}")]
    Transaction(sqlx::Error),
}

impl From<BasicStoreError> for JobStoreError {
    fn from(value: BasicStoreError) -> Self {
        JobStoreError::StoreBackendUnavailable(value.into())
    }
}
