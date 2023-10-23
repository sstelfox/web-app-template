use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use futures::Future;

use crate::jobs::{JobLike, Job, JobExecError, JobId, JobQueueError, JobState};

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

    async fn cancel(&self, id: JobId) -> Result<(), JobQueueError> {
        self.update_state(id, JobState::Cancelled).await
    }

    async fn enqueue<T: JobLike>(
        conn: &mut Self::Connection,
        task: T,
    ) -> Result<Option<JobId>, JobQueueError>
    where
        Self: Sized;

    async fn next(
        &self,
        queue_name: &str,
        task_names: &[&str],
    ) -> Result<Option<Job>, JobQueueError>;

    async fn retry(&self, id: JobId) -> Result<Option<JobId>, JobQueueError>;

    async fn update_state(&self, id: JobId, new_state: JobState) -> Result<(), JobQueueError>;
}
