use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use futures::Future;

use crate::background_jobs::{BackgroundJob, JobExecError, BackgroundJobId, BackgroundRunId, JobLike};
use crate::database::custom_types::BackgroundJobState;

pub(crate) type ExecuteJobFn<Context> = Arc<
    dyn Fn(
            serde_json::Value,
            Context,
        ) -> Pin<Box<dyn Future<Output = Result<(), JobExecError>> + Send>>
        + Send
        + Sync,
>;

pub(crate) type StateFn<Context> = Arc<dyn Fn() -> Context + Send + Sync>;

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

    async fn update_state(&self, id: BackgroundJobId, new_state: BackgroundJobState) -> Result<(), JobStoreError>;
}

#[derive(Debug, thiserror::Error)]
pub enum JobStoreError {
    #[error("unable to find job with ID {0}")]
    UnknownJob(BackgroundJobId),
}
