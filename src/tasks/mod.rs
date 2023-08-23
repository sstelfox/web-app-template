use std::collections::BTreeMap;
use std::fmt::{self, Display, Formatter};
use std::sync::Arc;

use axum::async_trait;
use serde::{Deserialize, Serialize};
use serde::de::DeserializeOwned;
use tokio::sync::{oneshot, Mutex};
use uuid::Uuid;

#[derive(Deserialize, Serialize)]
pub struct TestTask {
    pub number: usize,
}

#[derive(Clone, Debug)]
pub struct TaskContext;

impl TaskContext {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
pub trait TaskLike: Serialize + DeserializeOwned + Sync + Send {
    const MAX_RETRIES: usize = 3;
    const QUEUE_NAME: &'static str = "default";
    const TASK_NAME: &'static str;

    type TaskContext: Clone + Send;
    type Error: std::error::Error;

    async fn run(&self, ctx: Self::TaskContext) -> Result<(), Self::Error>;

    async fn unique_key(&self) -> Option<String> {
        None
    }
}

#[async_trait]
pub trait TaskLikeExt {
    async fn enqueue<S: TaskStore>(
        self,
        connection: &mut S::Connection,
    ) -> Result<(), TaskQueueError>;
}

#[derive(Debug)]
pub enum TaskQueueError {
    Unknown,
}

impl Display for TaskQueueError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let msg = match &self {
            TaskQueueError::Unknown => "unspecified error with the task queue",
        };

        f.write_str(msg)
    }
}

impl std::error::Error for TaskQueueError {}

#[async_trait]
pub trait TaskStore: Send + Sync + 'static {
    type Connection: Send;

    async fn cancel(&self, id: Uuid) -> Result<(), TaskQueueError> {
        self.update_state(id, TaskState::Cancelled).await
    }

    async fn enqueue<T: TaskLike>(
        conn: &mut Self::Connection,
        task: T,
    ) -> Result<(), TaskQueueError>
    where
        Self: Sized;

    async fn next(&self, queue_name: &str) -> Result<Option<Task>, TaskQueueError>;

    async fn reschedule(&self, id: Uuid, err: String) -> Result<(), TaskQueueError>;

    async fn update_state(&self, id: Uuid, state: TaskState) -> Result<(), TaskQueueError>;
}

#[derive(Debug)]
pub struct TaskError;

impl Display for TaskError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("unspecified error with the task")
    }
}

impl std::error::Error for TaskError {}

#[async_trait]
impl TaskLike for TestTask {
    const TASK_NAME: &'static str = "test_task";

    type TaskContext = TaskContext;
    type Error = TaskError;

    async fn run(&self, _ctx: Self::TaskContext) -> Result<(), Self::Error> {
        Ok(())
    }
}


#[derive(Eq, PartialEq)]
pub enum TaskState {
    New,
    InProgress,
    Cancelled,
    Failed,
    Complete,
    Dead,
}

pub struct Task {
    pub id: Uuid,

    pub name: String,
    queue_name: String,

    unique_key: Option<String>,
    state: TaskState,

    payload: serde_json::Value,
}

#[derive(Clone, Default)]
pub struct MemoryTaskStore {
    pub tasks: Arc<Mutex<BTreeMap<Uuid, Task>>>,
}

#[async_trait]
impl TaskStore for MemoryTaskStore {
    type Connection = Self;

    async fn enqueue<T: TaskLike>(
        conn: &mut Self::Connection,
        task: T,
    ) -> Result<(), TaskQueueError> {
        let unique_key = task.unique_key().await;
        let payload = serde_json::to_value(task)
            .map_err(|_| TaskQueueError::Unknown)?;

        let task = Task {
            id: Uuid::new_v4(),

            name: T::TASK_NAME.to_string(),
            queue_name: T::QUEUE_NAME.to_string(),

            unique_key,
            state: TaskState::New,

            payload,
        };

        let mut tasks = conn.tasks.lock().await;
        tasks.insert(task.id, task);

        Ok(())
    }

    async fn next(&self, queue_name: &str) -> Result<Option<Task>, TaskQueueError> {
        let mut tasks = self.tasks.lock().await;
        let mut next_task = None;

        for (_id, task) in tasks.iter().filter(|(_, task)| task.state == TaskState::New) {
            // todo: ordering, filtering based on queue all that goodness, maybe expiration
            // handling
        }

        Ok(next_task)
    }

    async fn reschedule(&self, id: Uuid, err: String) -> Result<(), TaskQueueError> {
        todo!()
    }

    async fn update_state(&self, id: Uuid, state: TaskState) -> Result<(), TaskQueueError> {
        todo!()
    }
}
