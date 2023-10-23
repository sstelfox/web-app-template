use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use futures::Future;

use crate::jobs::{JobLike, Task, TaskExecError, TaskId, TaskQueueError, TaskState};

pub(crate) type ExecuteTaskFn<Context> = Arc<
    dyn Fn(
            serde_json::Value,
            Context,
        ) -> Pin<Box<dyn Future<Output = Result<(), TaskExecError>> + Send>>
        + Send
        + Sync,
>;

pub(crate) type StateFn<Context> = Arc<dyn Fn() -> Context + Send + Sync>;

#[async_trait]
pub trait TaskStore: Send + Sync + 'static {
    type Connection: Send;

    async fn cancel(&self, id: TaskId) -> Result<(), TaskQueueError> {
        self.update_state(id, TaskState::Cancelled).await
    }

    async fn completed(&self, id: TaskId) -> Result<(), TaskQueueError> {
        self.update_state(id, TaskState::Complete).await
    }

    async fn enqueue<T: JobLike>(
        conn: &mut Self::Connection,
        task: T,
    ) -> Result<Option<TaskId>, TaskQueueError>
    where
        Self: Sized;

    async fn errored(
        &self,
        id: TaskId,
        error: TaskExecError,
    ) -> Result<Option<TaskId>, TaskQueueError> {
        use TaskExecError as TEE;

        match error {
            TEE::DeserializationFailed(_) | TEE::Panicked(_) => {
                self.update_state(id, TaskState::Dead).await?;
                Ok(None)
            }
            TEE::ExecutionFailed(_) => {
                self.update_state(id, TaskState::Error).await?;
                self.retry(id).await
            }
        }
    }

    async fn next(
        &self,
        queue_name: &str,
        task_names: &[&str],
    ) -> Result<Option<Task>, TaskQueueError>;

    async fn retry(&self, id: TaskId) -> Result<Option<TaskId>, TaskQueueError>;

    async fn update_state(&self, id: TaskId, state: TaskState) -> Result<(), TaskQueueError>;
}
