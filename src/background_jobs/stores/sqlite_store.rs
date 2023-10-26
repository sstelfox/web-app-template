use async_trait::async_trait;
use sqlx::SqlitePool;

use crate::background_jobs::stores::{JobStore, JobStoreError};
use crate::background_jobs::JobLike;
use crate::database::custom_types::{BackgroundJobId, BackgroundJobState, BackgroundRunId};
use crate::database::models::BackgroundJob;
use crate::database::Database;

#[derive(Clone)]
pub struct SqliteStore {
    database: Database,
}

impl SqliteStore {
    pub fn new(database: Database) -> Self {
        Self { database }
    }
}

#[async_trait]
impl JobStore for SqliteStore {
    type Connection = SqlitePool;

    //async fn cancel(&self, id: BackgroundJobId) -> Result<(), JobStoreError> {
    //    self.update_state(id, BackgroundJobState::Cancelled).await
    //}

    async fn enqueue<T: JobLike>(
        pool: &mut Self::Connection,
        task: T,
    ) -> Result<Option<(BackgroundJobId, BackgroundRunId)>, JobStoreError>
    where
        Self: Sized,
    {
        let mut conn = pool.acquire().await.map_err(SqliteStoreError::ConnError)?;
        let unique_key = task.unique_key().await;

        if let Some(key) = &unique_key {
            if key.is_active(&mut conn).await? {
                return Ok(None);
            }
        }

        let _transaction = pool.begin().await.map_err(SqliteStoreError::ConnError)?;

        todo!()
    }

    async fn next(
        &self,
        _queue_name: &str,
        _task_names: &[&str],
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
pub enum SqliteStoreError {
    #[error("failed to acquire connection from pool: {0}")]
    ConnError(sqlx::Error),

    #[error("an error occurred with a transaction operation: {0}")]
    TransactionError(sqlx::Error),
}

impl From<SqliteStoreError> for JobStoreError {
    fn from(value: SqliteStoreError) -> Self {
        JobStoreError::StoreBackendUnavailable(value.into())
    }
}
