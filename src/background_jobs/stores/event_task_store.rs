use async_trait::async_trait;
use sqlx::SqlitePool;

use crate::background_jobs::stores::{JobStore, JobStoreError};
use crate::background_jobs::JobLike;
use crate::database::custom_types::{BackgroundJobId, BackgroundJobState, BackgroundRunId};
use crate::database::models::BackgroundJob;

use crate::database::Database;
use crate::event_bus::EventBus;

#[derive(Clone)]
pub struct EventTaskContext {
    database: Database,
    event_bus: EventBus,
}

impl EventTaskContext {
    pub fn database(&self) -> &Database {
        &self.database
    }

    pub fn event_bus(&self) -> &EventBus {
        &self.event_bus
    }

    pub fn new(database: Database, event_bus: EventBus) -> Self {
        Self {
            database,
            event_bus,
        }
    }
}

#[derive(Clone)]
pub struct EventTaskStore {
    context: EventTaskContext,
}

impl EventTaskStore {
    pub fn context(&self) -> EventTaskContext {
        self.context.clone()
    }

    pub fn new(context: EventTaskContext) -> Self {
        Self { context }
    }
}

#[async_trait]
impl JobStore for EventTaskStore {
    type Connection = SqlitePool;

    //async fn cancel(&self, id: BackgroundJobId) -> Result<(), JobStoreError> {
    //    self.update_state(id, BackgroundJobState::Cancelled).await
    //}

    async fn enqueue<T: JobLike>(
        _pool: &mut Self::Connection,
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

#[derive(Debug, thiserror::Error)]
pub enum EventStoreError {
    #[error("failed to acquire connection from pool: {0}")]
    ConnError(sqlx::Error),

    #[error("an error occurred with a transaction operation: {0}")]
    TransactionError(sqlx::Error),
}

impl From<EventStoreError> for JobStoreError {
    fn from(value: EventStoreError) -> Self {
        JobStoreError::StoreBackendUnavailable(value.into())
    }
}
