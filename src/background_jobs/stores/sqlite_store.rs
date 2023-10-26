use async_trait::async_trait;
use sqlx::SqlitePool;

use crate::background_jobs::stores::{JobStore, JobStoreError};
use crate::background_jobs::JobLike;
use crate::database::custom_types::{BackgroundJobId, BackgroundJobState, BackgroundRunId};
use crate::database::models::BackgroundJob;

pub struct SqliteStore {
    pool: SqlitePool,
}

impl SqliteStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl JobStore for SqliteStore {
    type Connection = SqlitePool;

    //async fn cancel(&self, id: BackgroundJobId) -> Result<(), JobStoreError> {
    //    self.update_state(id, BackgroundJobState::Cancelled).await
    //}

    async fn enqueue<T: JobLike>(
        _conn: &mut Self::Connection,
        _task: T,
    ) -> Result<Option<(BackgroundJobId, BackgroundRunId)>, JobStoreError>
    where
        Self: Sized,
    {
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
