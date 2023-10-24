use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use futures::Future;

use crate::background_jobs::{BackgroundJob, JobExecError, BackgroundJobId, JobLike, JobQueueError};
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

    async fn cancel(&self, id: BackgroundJobId) -> Result<(), JobQueueError> {
        self.update_state(id, BackgroundJobState::Cancelled).await
    }

    async fn enqueue<T: JobLike>(
        conn: &mut Self::Connection,
        task: T,
    ) -> Result<Option<BackgroundJobId>, JobQueueError>
    where
        Self: Sized;

    async fn next(
        &self,
        queue_name: &str,
        task_names: &[&str],
    ) -> Result<Option<BackgroundJob>, JobQueueError>;

    async fn retry(&self, id: BackgroundJobId) -> Result<Option<BackgroundJobId>, JobQueueError>;

    async fn update_state(&self, id: BackgroundJobId, new_state: BackgroundJobState) -> Result<(), JobQueueError>;
}
