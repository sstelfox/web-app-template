pub(crate) mod sqlite_store;

use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use futures::Future;

use crate::background_jobs::{
    BackgroundJob, BackgroundJobId, BackgroundRunId, CaughtPanic, JobLike,
};
use crate::database::custom_types::BackgroundJobState;

pub(crate) type ExecuteJobFn<Context> = Arc<
    dyn Fn(
            serde_json::Value,
            Context,
        ) -> Pin<Box<dyn Future<Output = Result<(), JobExecError>> + Send>>
        + Send
        + Sync,
>;

#[derive(Debug, thiserror::Error)]
pub enum JobExecError {
    #[error("job deserialization failed: {0}")]
    DeserializationFailed(#[from] serde_json::Error),

    #[error("job execution failed: {0}")]
    ExecutionFailed(String),

    #[error("job panicked: {0}")]
    Panicked(#[from] CaughtPanic),
}

#[async_trait]
pub trait JobStore: Send + Sync + 'static {
    type Connection: Send;

    async fn cancel(&self, id: BackgroundJobId) -> Result<(), JobStoreError> {
        self.update_state(id, BackgroundJobState::Cancelled).await
    }

    async fn enqueue<T: JobLike>(
        conn: &mut Self::Connection,
        task: T,
    ) -> Result<Option<(BackgroundJobId, BackgroundRunId)>, JobStoreError>
    where
        Self: Sized;

    async fn next(
        &self,
        queue_name: &str,
        task_names: &[&str],
    ) -> Result<Option<BackgroundJob>, JobStoreError>;

    async fn retry(&self, id: BackgroundJobId) -> Result<Option<BackgroundRunId>, JobStoreError>;

    async fn update_state(
        &self,
        id: BackgroundJobId,
        new_state: BackgroundJobState,
    ) -> Result<(), JobStoreError>;
}

#[derive(Debug, thiserror::Error)]
pub enum JobStoreError {
    #[error("detected corruption in database: {0}")]
    DataCorruption(Box<dyn std::error::Error>),

    #[error("the store backend experienced an error: {0}")]
    StoreBackendUnavailable(Box<dyn std::error::Error>),

    #[error("unable to find job with ID {0}")]
    UnknownJob(BackgroundJobId),
}

pub(crate) type StateFn<Context> = Arc<dyn Fn() -> Context + Send + Sync>;
