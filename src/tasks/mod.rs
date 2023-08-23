use std::collections::BTreeMap;
use std::fmt::{self, Display, Formatter};
use std::sync::Arc;

use axum::async_trait;
use serde::{Deserialize, Serialize};
use serde::de::DeserializeOwned;
use tokio::sync::{oneshot, Mutex};

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

    async fn enqueue<T: TaskLike>(
        conn: &mut Self::Connection,
        task: T,
    ) -> Result<(), TaskQueueError>
    where
        Self: Sized;

    async fn next_task(&self, queue_name: &str) -> Result<Option<Task>, TaskQueueError>;
    async fn set_task_state(&self, id: String, state: TaskState) -> Result<(), TaskQueueError>;
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


pub enum TaskState {
    New,
    InProgress,
    Cancelled,
    Failed,
    Complete,
    Dead,
}

pub struct Task {
    pub id: String,

    pub name: String,
    queue_name: String,

    uniq_hash: Option<String>,

    payload: serde_json::Value,

    // these should be some kind of dates but I'm ignore that for now
    created_at: String,
    started_at: String,
}

#[derive(Clone, Default)]
pub struct MemoryTaskStore {
    pub tasks: Arc<Mutex<BTreeMap<String, Task>>>,
}

//impl TaskStore for MemoryTaskStore {
//}
