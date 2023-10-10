use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt::{self, Debug, Display, Formatter};
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use axum::async_trait;
use futures::Future;
use itertools::Itertools;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tokio::sync::Mutex;
use uuid::Uuid;

const TASK_EXECUTION_TIMEOUT: Duration = Duration::from_secs(30);

pub type ExecuteTaskFn<Context> = Arc<
    dyn Fn(
        CurrentTask,
        serde_json::Value,
        Context,
    ) -> Pin<Box<dyn Future<Output = Result<(), TaskExecError>> + Send>>
    + Send
    + Sync,
>;

pub type StateFn<Context> = Arc<dyn Fn() -> Context + Send + Sync>;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct QueueConfig {
    name: String,
    num_workers: usize,
}

impl QueueConfig {
    pub fn new(name: impl ToString) -> Self {
        Self {
            name: name.to_string(),
            num_workers: 1,
        }
    }

    pub fn num_workers(mut self, num_workers: usize) -> Self {
        self.num_workers = num_workers;
        self
    }
}

impl<S> From<S> for QueueConfig
where
    S: ToString,
{
    fn from(name: S) -> Self {
        Self::new(name.to_string())
    }
}

#[async_trait]
pub trait TaskLike: Serialize + DeserializeOwned + Sync + Send + 'static {
    const MAX_RETRIES: usize = 3;

    const QUEUE_NAME: &'static str = "default";

    const TASK_NAME: &'static str;

    type Error: std::error::Error;
    type Context: Clone + Send + 'static;

    async fn run(&self, task: CurrentTask, ctx: Self::Context) -> Result<(), Self::Error>;

    async fn unique_key(&self) -> Option<String> {
        None
    }
}

#[async_trait]
pub trait TaskLikeExt {
    async fn enqueue<S: TaskStore>(
        self,
        connection: &mut S::Connection,
    ) -> Result<Option<TaskId>, TaskQueueError>;
}

#[async_trait]
impl<T> TaskLikeExt for T
where
    T: TaskLike,
{
    async fn enqueue<S: TaskStore>(
        self,
        connection: &mut S::Connection,
    ) -> Result<Option<TaskId>, TaskQueueError> {
        S::enqueue(connection, self).await
    }
}

pub struct CreateTask {
    name: String,
    queue_name: String,

    payload: serde_json::Value,
    maximum_attempts: usize,

    scheduled_to_run_at: OffsetDateTime,
}

pub struct CurrentTask {
    id: TaskId,
    current_attempt: usize,
    scheduled_at: OffsetDateTime,
    started_at: OffsetDateTime,
}

impl CurrentTask {
    pub fn new(task: &Task) -> Self {
        Self {
            id: task.id,
            current_attempt: task.current_attempt,
            scheduled_at: task.scheduled_at,
            started_at: task.started_at.expect("task to be started"),
        }
    }
}

#[derive(Clone, Copy, Hash, Eq, Ord, PartialEq, PartialOrd, Serialize, sqlx::Type)]
#[sqlx(transparent)]
pub struct TaskId(Uuid);

impl Debug for TaskId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_tuple("TaskId").field(&self.0).finish()
    }
}

impl Display for TaskId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for TaskId {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

impl From<TaskId> for Uuid {
    fn from(value: TaskId) -> Self {
        value.0
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TaskQueueError {
    #[error("unable to find task with ID {0}")]
    UnknownTask(TaskId),

    #[error("unspecified error with the task queue")]
    Unknown,
}

#[async_trait]
pub trait TaskStore: Send + Sync + 'static {
    type Connection: Send;

    async fn cancel(&self, id: TaskId) -> Result<(), TaskQueueError> {
        self.update_state(id, TaskState::Cancelled).await
    }

    async fn enqueue<T: TaskLike>(
        conn: &mut Self::Connection,
        task: T,
    ) -> Result<Option<TaskId>, TaskQueueError>
    where
        Self: Sized;

    async fn next(&self, queue_name: &str) -> Result<Option<Task>, TaskQueueError>;

    async fn enqueue_retry(&self, id: TaskId) -> Result<Option<TaskId>, TaskQueueError>;

    async fn update_state(&self, id: TaskId, state: TaskState) -> Result<(), TaskQueueError>;
}

#[derive(Clone, Debug, PartialEq)]
pub struct Task {
    pub id: TaskId,

    next_id: Option<TaskId>,
    previous_id: Option<TaskId>,

    name: String,
    queue_name: String,

    unique_key: Option<String>,
    state: TaskState,

    current_attempt: usize,
    maximum_attempts: usize,

    // will need a live-cancel signal and likely a custom Future impl to ensure its used for proper
    // timeout handling

    payload: serde_json::Value,
    error: Option<serde_json::Value>,

    scheduled_at: OffsetDateTime,
    scheduled_to_run_at: OffsetDateTime,

    started_at: Option<OffsetDateTime>,
    finished_at: Option<OffsetDateTime>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskState {
    New,
    InProgress,
    Retry,
    Cancelled,
    Error,
    Complete,
    TimedOut,
    Dead,
}

#[derive(Clone, Default)]
pub struct MemoryTaskStore {
    pub tasks: Arc<Mutex<BTreeMap<TaskId, Task>>>,
}

impl MemoryTaskStore {
    // note: might want to extend this to be unique over a queue... I could just prepend the queue
    // the key or something...
    async fn is_key_present(conn: &Self, key: &str) -> bool {
        let tasks = conn.tasks.lock().await;

        for (_, task) in tasks.iter() {
            // we only need to look at a task if it also has a unique key
            let existing_key = match &task.unique_key {
                Some(ek) => ek,
                None => continue,
            };

            // any task that has already ended isn't considered for uniqueness checks
            if !matches!(task.state, TaskState::New | TaskState::InProgress) {
                continue;
            }

            // we found a match, we don't need to enqueue a new task
            if key == existing_key {
                return true;
            }
        }

        false
    }
}

#[async_trait]
impl TaskStore for MemoryTaskStore {
    type Connection = Self;

    async fn enqueue<T: TaskLike>(
        conn: &mut Self::Connection,
        task: T,
    ) -> Result<Option<TaskId>, TaskQueueError> {
        let unique_key = task.unique_key().await;

        if let Some(new_key) = &unique_key {
            if MemoryTaskStore::is_key_present(conn, new_key).await {
                return Ok(None);
            }
        }

        let id = TaskId::from(Uuid::new_v4());
        let payload = serde_json::to_value(task).map_err(|_| TaskQueueError::Unknown)?;

        let task = Task {
            id,

            next_id: None,
            previous_id: None,

            name: T::TASK_NAME.to_string(),
            queue_name: T::QUEUE_NAME.to_string(),

            unique_key,
            state: TaskState::New,
            current_attempt: 0,
            maximum_attempts: T::MAX_RETRIES,

            payload,
            error: None,

            scheduled_at: OffsetDateTime::now_utc(),
            scheduled_to_run_at: OffsetDateTime::now_utc(),

            started_at: None,
            finished_at: None,
        };

        let mut tasks = conn.tasks.lock().await;
        tasks.insert(task.id, task);

        Ok(Some(id))
    }

    async fn enqueue_retry(&self, id: TaskId) -> Result<Option<TaskId>, TaskQueueError> {
        let mut tasks = self.tasks.lock().await;

        let target_task = match tasks.get_mut(&id) {
            Some(t) => t,
            None => return Err(TaskQueueError::UnknownTask(id)),
        };

        // these states are the only retryable states
        if !matches!(target_task.state, TaskState::Error | TaskState::TimedOut) {
            tracing::warn!(?id, "task is not in a state that can be retried");
            return Err(TaskQueueError::Unknown);
        }

        // no retries remaining mark the task as dead
        if target_task.current_attempt >= target_task.maximum_attempts {
            tracing::warn!(?id, "task failed with no more attempts remaining");
            target_task.state = TaskState::Dead;
            return Ok(None);
        }

        let mut new_task = target_task.clone();

        let new_id = TaskId::from(Uuid::new_v4());
        target_task.next_id = Some(new_task.id);

        new_task.id = new_id;
        new_task.previous_id = Some(target_task.id);

        new_task.current_attempt += 1;
        new_task.state = TaskState::Retry;
        new_task.scheduled_at = OffsetDateTime::now_utc();
        // for now just retry again in five minutes, will probably want some kind of backoff for
        // this
        new_task.scheduled_to_run_at = OffsetDateTime::now_utc() + Duration::from_secs(300);

        tasks.insert(new_task.id, new_task);

        tracing::info!(?id, ?new_id, "task will be retried in the future");

        Ok(Some(new_id))
    }

    async fn next(&self, queue_name: &str) -> Result<Option<Task>, TaskQueueError> {
        let mut tasks = self.tasks.lock().await;
        let mut next_task = None;

        let reference_time = OffsetDateTime::now_utc();
        let mut tasks_to_retry = Vec::new();

        for (id, task) in tasks
            .iter_mut()
            .filter(|(_, task)| task.scheduled_to_run_at <= reference_time)
            .sorted_by(|a, b| sort_tasks(a.1, b.1))
        {
            match (task.state, task.started_at) {
                (TaskState::New, None) => {
                    if task.queue_name != queue_name {
                        continue;
                    }

                    task.started_at = Some(OffsetDateTime::now_utc());
                    task.state = TaskState::InProgress;

                    next_task = Some(task.clone());
                    break;
                }
                (TaskState::InProgress, Some(started_at)) => {
                    if (started_at + TASK_EXECUTION_TIMEOUT) >= OffsetDateTime::now_utc() {
                        // todo: need to send cancel signal to the task
                        task.state = TaskState::TimedOut;
                        task.finished_at = Some(OffsetDateTime::now_utc());

                        tasks_to_retry.push(id);

                        continue;
                    }
                }
                // cancelled is the only other state allowed to not have a started_at
                (TaskState::Cancelled, None) => (),
                (state, None) => {
                    tracing::error!(id = ?task.id, ?state, "encountered task in illegal state");

                    task.state = TaskState::Error;
                    task.finished_at = Some(OffsetDateTime::now_utc());
                }
                _ => (),
            }
        }

        for id in tasks_to_retry.into_iter() {
            // this is best effort mostly for timed out
            let _ = self.enqueue_retry(*id).await;
        }

        Ok(next_task)
    }

    async fn update_state(&self, id: TaskId, new_state: TaskState) -> Result<(), TaskQueueError> {
        let mut tasks = self.tasks.lock().await;

        let task = match tasks.get_mut(&id) {
            Some(t) => t,
            None => return Err(TaskQueueError::UnknownTask(id)),
        };

        if task.state != TaskState::InProgress {
            tracing::error!("only in progress tasks are allowed to transition to other states");
            return Err(TaskQueueError::Unknown);
        }

        match new_state {
            // this state should only exist when the task is first created
            TaskState::New => {
                tracing::error!("can't transition an existing task to the New state");
                return Err(TaskQueueError::Unknown);
            }
            // this is an internal transition that happens automatically when the task is picked up
            TaskState::InProgress => {
                tracing::error!(
                    "only the task store may transition a task to the InProgress state"
                );
                return Err(TaskQueueError::Unknown);
            }
            _ => (),
        }

        task.finished_at = Some(OffsetDateTime::now_utc());
        task.state = new_state;

        Ok(())
    }
}

fn sort_tasks(a: &Task, b: &Task) -> Ordering {
    match a.scheduled_to_run_at.cmp(&b.scheduled_to_run_at) {
        Ordering::Equal => a.scheduled_at.cmp(&b.scheduled_at),
        ord => ord,
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TaskExecError {
    #[error("task deserialization failed: {0}")]
    TaskDeserializationFailed(#[from] serde_json::Error),

    #[error("task execution failed: {0}")]
    ExecutionFailed(String),

    #[error("task panicked with: {0}")]
    Panicked(String),
}

#[derive(Clone)]
pub struct WorkerPool<Context, S>
where
    Context: Clone + Send + 'static,
    S: TaskStore + Clone,
{
    task_store: S,

    application_data_fn: StateFn<Context>,

    task_registry: BTreeMap<&'static str, ExecuteTaskFn<Context>>,

    queue_tasks: BTreeMap<&'static str, Vec<&'static str>>,

    worker_queues: BTreeMap<String, QueueConfig>,
}

impl<Context, S> WorkerPool<Context, S>
where
    Context: Clone + Send + 'static,
    S: TaskStore + Clone,
{
    pub fn new<A>(task_store: S, application_data_fn: A) -> Self
    where
        A: Fn() -> Context + Send + Sync + 'static,
    {
        Self {
            task_store,
            application_data_fn: Arc::new(application_data_fn),
            task_registry: BTreeMap::new(),
            queue_tasks: BTreeMap::new(),
            worker_queues:BTreeMap::new(),
        }
    }

    pub fn register_task_type<TL>(mut self) -> Self
    where
        TL: TaskLike<Context = Context>,
    {
        self.queue_tasks
            .entry(TL::QUEUE_NAME)
            .or_insert_with(Vec::new)
            .push(TL::TASK_NAME);

        self.task_registry
            .insert(TL::TASK_NAME, Arc::new(deserialize_and_run_task::<TL>));

        self
    }
}

fn deserialize_and_run_task<TL>(
    current_task: CurrentTask,
    payload: serde_json::Value,
    context: TL::Context,
) -> Pin<Box<dyn Future<Output = Result<(), TaskExecError>> + Send>>
where
    TL: TaskLike,
{
    Box::pin(async move {
        let task: TL = serde_json::from_value(payload)?;

        match task.run(current_task, context).await {
            Ok(_) => Ok(()),
            Err(err) => Err(TaskExecError::ExecutionFailed(err.to_string())),
        }
    })
}

// example specific task implementation, everything above is supporting infrastructure

#[derive(Deserialize, Serialize)]
pub struct TestTask {
    number: usize,
}

impl TestTask {
    pub fn new(number: usize) -> Self {
        Self { number }
    }
}

#[async_trait]
impl TaskLike for TestTask {
    const TASK_NAME: &'static str = "test_task";

    type Error = TestTaskError;
    type Context = ();

    async fn run(&self, _task: CurrentTask, _ctx: Self::Context) -> Result<(), Self::Error> {
        tracing::info!("the test task value is {}", self.number);
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TestTaskError {
    #[error("unspecified error with the task")]
    Unknown,
}
