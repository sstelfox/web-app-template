#![allow(dead_code)]

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::ops::{Deref, DerefMut};
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use axum::async_trait;
use futures::future::join_all;
use futures::Future;
use itertools::Itertools;
use serde::de::DeserializeOwned;
use serde::Serialize;
use time::OffsetDateTime;
use tokio::sync::{watch, Mutex};
use tokio::task::JoinHandle;
use tokio::time::timeout;
use uuid::Uuid;

mod catch_panic_future;
pub mod impls;
mod interface;
mod job_id;
mod queue_config;
mod stores;

use catch_panic_future::{CatchPanicFuture, CaughtPanic};
pub use queue_config::QueueConfig;
use job_id::JobId;
use stores::{ExecuteJobFn, StateFn, JobStore};

const JOB_EXECUTION_TIMEOUT: Duration = Duration::from_secs(30);

const MAXIMUM_CHECK_DELAY: Duration = Duration::from_millis(250);

const WORKER_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

#[async_trait]
pub trait JobLike: Serialize + DeserializeOwned + Sync + Send + 'static {
    // todo: rename MAX_ATTEMPTS
    const MAX_RETRIES: usize = 3;

    const QUEUE_NAME: &'static str = "default";

    const JOB_NAME: &'static str;

    type Error: std::error::Error;
    type Context: Clone + Send + 'static;

    async fn run(&self, ctx: Self::Context) -> Result<(), Self::Error>;

    async fn unique_key(&self) -> Option<String> {
        None
    }
}

#[async_trait]
pub trait JobLikeExt {
    async fn enqueue<S: JobStore>(
        self,
        connection: &mut S::Connection,
    ) -> Result<Option<JobId>, JobQueueError>;
}

#[async_trait]
impl<T> JobLikeExt for T
where
    T: JobLike,
{
    async fn enqueue<S: JobStore>(
        self,
        connection: &mut S::Connection,
    ) -> Result<Option<JobId>, JobQueueError> {
        S::enqueue(connection, self).await
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Job {
    pub id: JobId,

    next_id: Option<JobId>,
    previous_id: Option<JobId>,

    name: String,
    queue_name: String,

    unique_key: Option<String>,
    state: JobState,

    current_attempt: usize,
    maximum_attempts: usize,

    // will need a live-cancel signal and likely a custom Future impl to ensure its used for proper
    // timeout handling

    // todo: maybe this should be an Option so I can clear it once the job is completed
    // successfully...
    payload: serde_json::Value,
    error: Option<String>,

    scheduled_at: OffsetDateTime,
    scheduled_to_run_at: OffsetDateTime,

    started_at: Option<OffsetDateTime>,
    finished_at: Option<OffsetDateTime>,
}

#[derive(Debug, thiserror::Error)]
pub enum JobExecError {
    #[error("job deserialization failed: {0}")]
    DeserializationFailed(#[from] serde_json::Error),

    #[error("job execution failed: {0}")]
    ExecutionFailed(String),

    #[error("job panicked: {0}")]
    Panicked(#[from] CaughtPanic),
}

#[derive(Debug, thiserror::Error)]
pub enum JobQueueError {
    #[error("unable to find job with ID {0}")]
    UnknownJob(JobId),

    #[error("I lazily hit one of the queue errors I haven't implemented yet")]
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JobState {
    New,
    InProgress,
    Panicked,
    Retry,
    Cancelled,
    Error,
    Complete,
    TimedOut,
    Dead,
}

struct Worker<Context, S>
where
    Context: Clone + Send + 'static,
    S: JobStore + Clone,
{
    name: String,
    queue_config: QueueConfig,

    context_data_fn: StateFn<Context>,
    store: S,
    job_registry: BTreeMap<&'static str, ExecuteJobFn<Context>>,

    shutdown_signal: Option<tokio::sync::watch::Receiver<()>>,
}

impl<Context, S> Worker<Context, S>
where
    Context: Clone + Send + 'static,
    S: JobStore + Clone,
{
    fn new(
        name: String,
        queue_config: QueueConfig,
        context_data_fn: StateFn<Context>,
        store: S,
        job_registry: BTreeMap<&'static str, ExecuteJobFn<Context>>,
        shutdown_signal: Option<tokio::sync::watch::Receiver<()>>,
    ) -> Self {
        Self {
            name,
            queue_config,
            context_data_fn,
            store,
            job_registry,
            shutdown_signal,
        }
    }

    async fn run(&self, job: Job) -> Result<(), WorkerError> {
        let deserialize_and_run_job_fn = self
            .job_registry
            .get(job.name.as_str())
            .ok_or(WorkerError::UnregisteredJobName(job.name))?
            .clone();

        let safe_runner = CatchPanicFuture::wrap({
            let context = (self.context_data_fn)();
            let payload = job.payload.clone();

            async move { deserialize_and_run_job_fn(payload, context).await }
        });

        // an error here occurs only when the job panicks, deserialization and regular job
        // execution errors are handled next
        //
        // todo: should note the job as having panicked if that's why this failed. There is also a
        // chance that the worker is corrupted in some way by the panic so I should set a flag on
        // this worker and handle two consecutive panics as a worker problem. The second job
        // triggering the panic should be presumed innocent and restored to a runnable state.
        let job_result = match safe_runner.await {
            Ok(tr) => tr,
            Err(err) => {
                tracing::error!("job panicked: {err}");

                // todo: save panic message into the job.error and save it back to the memory
                // store somehow...
                self.store
                    .update_state(job.id, JobState::Panicked)
                    .await
                    .map_err(WorkerError::UpdateJobStatusFailed)?;

                // we didn't complete successfully, but we do want to keep processing jobs for
                // now. We may be corrupted due to the panic somehow if additional errors crop up.
                // Left as future work to handle this edge case.
                return Ok(());
            }
        };

        match job_result {
            Ok(_) => {
                self.store
                    .update_state(job.id, JobState::Complete)
                    .await
                    .map_err(WorkerError::UpdateJobStatusFailed)?;
            }
            Err(err) => {
                tracing::error!("job failed with error: {err}");

                self.store
                    .update_state(job.id, JobState::Error)
                    .await
                    .map_err(WorkerError::UpdateJobStatusFailed)?;

                self.store
                    .retry(job.id)
                    .await
                    .map_err(WorkerError::RetryJobFailed)?;
            }
        }

        Ok(())
    }

    async fn run_jobs(&mut self) -> Result<(), WorkerError> {
        let relevant_job_names: Vec<&'static str> = self.job_registry.keys().cloned().collect();

        loop {
            // check to see if its time to shutdown the worker
            //
            // todo: turn this into a select with a short fallback timeout on job execution to try
            // and finish it within our graceful shutdown window
            if let Some(shutdown_signal) = &self.shutdown_signal {
                match shutdown_signal.has_changed() {
                    Ok(true) => return Ok(()),
                    Err(_) => return Err(WorkerError::EmergencyShutdown),
                    _ => (),
                }
            }

            let next_job = self
                .store
                .next(self.queue_config.name(), &relevant_job_names)
                .await
                .map_err(WorkerError::StoreUnavailable)?;

            if let Some(job) = next_job {
                tracing::info!(id = ?job.id, "starting execution of job");
                self.run(job).await?;
                continue;
            }

            // todo this should probably be handled by some form of a centralized wake up manager
            // when things are enqueued which can also 'alarm' when a pending job is ready to be
            // scheduled instead of relying... and that change should probably be done using
            // future wakers instead of internal timeouts but some central scheduler
            match &mut self.shutdown_signal {
                Some(ss) => {
                    if let Ok(_signaled) =
                        tokio::time::timeout(MAXIMUM_CHECK_DELAY, ss.changed()).await
                    {
                        // todo might want to handle graceful / non-graceful differently
                        tracing::info!("received worker shutdown signal while idle");
                        return Ok(());
                    }

                    // intentionally letting the 'error' type fall through here as it means we
                    // timed out on waiting for a shutdown signal and should continue
                }
                None => {
                    tracing::info!("no jobs available for worker, sleeping for a time...");
                    let _ = tokio::time::sleep(MAXIMUM_CHECK_DELAY).await;
                }
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WorkerError {
    #[error("worker detected an error in the shutdown channel and forced and immediate exit")]
    EmergencyShutdown,

    #[error("failed to enqueue a failed job for re-execution: {0}")]
    RetryJobFailed(JobQueueError),

    #[error("error while attempting to retrieve the next job: {0}")]
    StoreUnavailable(JobQueueError),

    #[error("failed to update job status with store: {0}")]
    UpdateJobStatusFailed(JobQueueError),

    #[error("during execution of a dequeued job, encountered unregistered job '{0}'")]
    UnregisteredJobName(String),
}

#[derive(Clone)]
pub struct WorkerPool<Context, S>
where
    Context: Clone + Send + 'static,
    S: JobStore + Clone,
{
    context_data_fn: StateFn<Context>,
    job_store: S,
    job_registry: BTreeMap<&'static str, ExecuteJobFn<Context>>,

    queue_jobs: BTreeMap<&'static str, Vec<&'static str>>,
    worker_queues: BTreeMap<&'static str, QueueConfig>,
}

impl<Context, S> WorkerPool<Context, S>
where
    Context: Clone + Send + 'static,
    S: JobStore + Clone,
{
    pub fn configure_queue(mut self, config: QueueConfig) -> Self {
        self.worker_queues.insert(config.name(), config);
        self
    }

    pub fn new<A>(job_store: S, context_data_fn: A) -> Self
    where
        A: Fn() -> Context + Send + Sync + 'static,
    {
        Self {
            context_data_fn: Arc::new(context_data_fn),
            job_store,
            job_registry: BTreeMap::new(),

            queue_jobs: BTreeMap::new(),
            worker_queues: BTreeMap::new(),
        }
    }

    pub fn register_job_type<TL>(mut self) -> Self
    where
        TL: JobLike<Context = Context>,
    {
        self.queue_jobs
            .entry(TL::QUEUE_NAME)
            .or_default()
            .push(TL::JOB_NAME);

        self.job_registry
            .insert(TL::JOB_NAME, Arc::new(deserialize_and_run_job::<TL>));

        self
    }

    pub async fn start<F>(self, shutdown_signal: F) -> Result<JoinHandle<()>, WorkerPoolError>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        for (queue_name, queue_tracked_jobs) in self.queue_jobs.iter() {
            if !self.worker_queues.contains_key(queue_name) {
                return Err(WorkerPoolError::QueueNotConfigured(
                    queue_name,
                    queue_tracked_jobs.clone(),
                ));
            }
        }

        let (inner_shutdown_tx, inner_shutdown_rx) = watch::channel(());
        let mut worker_handles = Vec::new();

        for (queue_name, queue_config) in self.worker_queues.iter() {
            for idx in 0..(queue_config.worker_count()) {
                let worker_name = format!("worker-{queue_name}-{idx}");

                // todo: make the worker_name into a span attached to this future and drop it from
                // the worker attributes

                let mut worker: Worker<Context, S> = Worker::new(
                    worker_name.clone(),
                    queue_config.clone(),
                    self.context_data_fn.clone(),
                    self.job_store.clone(),
                    self.job_registry.clone(),
                    Some(inner_shutdown_rx.clone()),
                );

                let worker_handle = tokio::spawn(async move {
                    match worker.run_jobs().await {
                        Ok(()) => {
                            tracing::info!(name = ?worker_name, "worker stopped successfully")
                        }
                        Err(err) => {
                            tracing::error!(name = ?worker_name, "worker stopped due to error: {err}")
                        }
                    }
                });

                worker_handles.push(worker_handle);
            }
        }

        let shutdown_guard = tokio::spawn(async move {
            // Wait until we receive a shutdown signal directly or the channel errors out due to
            // the other side being dropped
            let _ = shutdown_signal.await;

            // In either case, its time to shut things down. Let's try and notify our workers for
            // graceful shutdown.
            let _ = inner_shutdown_tx.send(());

            // try and collect error from workers but if it takes too long abandon them
            let worker_errors: Vec<_> = match timeout(
                WORKER_SHUTDOWN_TIMEOUT,
                join_all(worker_handles),
            )
            .await
            {
                Ok(res) => res
                    .into_iter()
                    .filter(Result::is_err)
                    .map(Result::unwrap_err)
                    .collect(),
                Err(_) => {
                    tracing::warn!("timed out waiting for workers to shutdown, not reporting outstanding errors");
                    Vec::new()
                }
            };

            if worker_errors.is_empty() {
                tracing::info!("worker pool shutdown gracefully");
            } else {
                tracing::error!(
                    "workers reported the following errors during shutdown:\n{:?}",
                    worker_errors
                );
            }
        });

        Ok(shutdown_guard)
    }
}

#[derive(Clone)]
pub struct WorkScheduler<T: JobStore>(T);

impl<T: JobStore> WorkScheduler<T> {
    pub fn new(store: T) -> Self {
        Self(store)
    }
}

impl<T: JobStore> Deref for WorkScheduler<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: JobStore> DerefMut for WorkScheduler<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WorkSchedulerError {
    #[error("failed to enqueue job to workers: {0}")]
    EnqueueFailed(JobQueueError),
}

#[derive(Debug, thiserror::Error)]
pub enum WorkerPoolError {
    #[error("found named queue '{0}' defined by job(s) {1:?} that doesn't have a matching queue config")]
    QueueNotConfigured(&'static str, Vec<&'static str>),
}

// local helper functions

fn deserialize_and_run_job<JL>(
    payload: serde_json::Value,
    context: JL::Context,
) -> Pin<Box<dyn Future<Output = Result<(), JobExecError>> + Send>>
where
    JL: JobLike,
{
    Box::pin(async move {
        let job: JL = serde_json::from_value(payload)?;

        match job.run(context).await {
            Ok(_) => Ok(()),
            Err(err) => Err(JobExecError::ExecutionFailed(err.to_string())),
        }
    })
}

fn sort_jobs(a: &Job, b: &Job) -> Ordering {
    match a.scheduled_to_run_at.cmp(&b.scheduled_to_run_at) {
        Ordering::Equal => a.scheduled_at.cmp(&b.scheduled_at),
        ord => ord,
    }
}

// concrete work store implementation

#[derive(Clone, Default)]
pub struct MemoryJobStore {
    pub jobs: Arc<Mutex<BTreeMap<JobId, Job>>>,
}

impl MemoryJobStore {
    // note: might want to extend this to be unique over a queue... I could just prepend the queue
    // the key or something...
    async fn is_key_present(conn: &Self, key: &str) -> bool {
        let jobs = conn.jobs.lock().await;

        for (_, job) in jobs.iter() {
            // we only need to look at a job if it also has a unique key
            let existing_key = match &job.unique_key {
                Some(ek) => ek,
                None => continue,
            };

            // any job that has already ended isn't considered for uniqueness checks
            if !matches!(
                job.state,
                JobState::New | JobState::InProgress | JobState::Retry
            ) {
                continue;
            }

            // we found a match, we don't need to enqueue a new job
            if key == existing_key {
                return true;
            }
        }

        false
    }
}

#[async_trait]
impl JobStore for MemoryJobStore {
    type Connection = Self;

    async fn enqueue<T: JobLike>(
        conn: &mut Self::Connection,
        job: T,
    ) -> Result<Option<JobId>, JobQueueError> {
        let unique_key = job.unique_key().await;

        if let Some(new_key) = &unique_key {
            if MemoryJobStore::is_key_present(conn, new_key).await {
                return Ok(None);
            }
        }

        let id = JobId::from(Uuid::new_v4());
        let payload = serde_json::to_value(job).map_err(|_| JobQueueError::Unknown)?;

        let job = Job {
            id,

            next_id: None,
            previous_id: None,

            name: T::JOB_NAME.to_string(),
            queue_name: T::QUEUE_NAME.to_string(),

            unique_key,
            state: JobState::New,
            current_attempt: 0,
            maximum_attempts: T::MAX_RETRIES,

            payload,
            error: None,

            scheduled_at: OffsetDateTime::now_utc(),
            scheduled_to_run_at: OffsetDateTime::now_utc(),

            started_at: None,
            finished_at: None,
        };

        let mut jobs = conn.jobs.lock().await;
        jobs.insert(job.id, job);

        Ok(Some(id))
    }

    async fn next(
        &self,
        queue_name: &str,
        job_names: &[&str],
    ) -> Result<Option<Job>, JobQueueError> {
        let mut jobs = self.jobs.lock().await;
        let mut next_job = None;

        let reference_time = OffsetDateTime::now_utc();
        let mut jobs_to_retry = Vec::new();

        for (id, job) in jobs
            .iter_mut()
            .filter(|(_, job)| {
                job_names.contains(&job.name.as_str())
                    && job.scheduled_to_run_at <= reference_time
            })
            // only care about jobs that have a state to advance
            .filter(|(_, job)| {
                matches!(
                    job.state,
                    JobState::New | JobState::InProgress | JobState::Retry
                )
            })
            .sorted_by(|a, b| sort_jobs(a.1, b.1))
        {
            match (job.state, job.started_at) {
                (JobState::New | JobState::Retry, None) => {
                    if job.queue_name != queue_name {
                        continue;
                    }

                    job.started_at = Some(OffsetDateTime::now_utc());
                    job.state = JobState::InProgress;

                    next_job = Some(job.clone());
                    break;
                }
                (JobState::InProgress, Some(started_at)) => {
                    if (started_at + JOB_EXECUTION_TIMEOUT) >= OffsetDateTime::now_utc() {
                        // todo: need to send cancel signal to the job
                        job.state = JobState::TimedOut;
                        job.finished_at = Some(OffsetDateTime::now_utc());

                        jobs_to_retry.push(id);
                    }
                }
                (state, _) => {
                    tracing::error!(id = ?job.id, ?state, "encountered job in illegal state");
                    job.state = JobState::Dead;
                    job.finished_at = Some(OffsetDateTime::now_utc());
                }
            }
        }

        for id in jobs_to_retry.into_iter() {
            // attempt to requeue any of these jobs we encountered, if we fail to requeue them its
            // not a big deal but we will keep trying if they stay in that state... Might want to
            // put some kind of time window on these or something
            let _ = self.retry(*id).await;
        }

        Ok(next_job)
    }

    async fn retry(&self, id: JobId) -> Result<Option<JobId>, JobQueueError> {
        let mut jobs = self.jobs.lock().await;

        let target_job = match jobs.get_mut(&id) {
            Some(t) => t,
            None => return Err(JobQueueError::UnknownJob(id)),
        };

        // these states are the only retryable states
        if !matches!(target_job.state, JobState::Error | JobState::TimedOut) {
            tracing::warn!(?id, "job is not in a state that can be retried");
            return Err(JobQueueError::Unknown);
        }

        // no retries remaining mark the job as dead
        if target_job.current_attempt >= target_job.maximum_attempts {
            tracing::warn!(?id, "job failed with no more attempts remaining");
            target_job.state = JobState::Dead;
            return Ok(None);
        }

        let mut new_job = target_job.clone();

        let new_id = JobId::from(Uuid::new_v4());
        target_job.next_id = Some(new_job.id);

        new_job.id = new_id;
        new_job.previous_id = Some(target_job.id);

        new_job.current_attempt += 1;
        new_job.state = JobState::Retry;
        new_job.started_at = None;
        new_job.scheduled_at = OffsetDateTime::now_utc();

        // really rough exponential backoff, 4, 8, and 16 seconds by default
        let backoff_secs = 2u64.saturating_pow(new_job.current_attempt.saturating_add(1) as u32);
        tracing::info!(
            ?id,
            ?new_id,
            "job will be retried {backoff_secs} secs in the future"
        );
        new_job.scheduled_to_run_at =
            OffsetDateTime::now_utc() + Duration::from_secs(backoff_secs);

        jobs.insert(new_job.id, new_job);

        Ok(Some(new_id))
    }

    async fn update_state(&self, id: JobId, new_state: JobState) -> Result<(), JobQueueError> {
        let mut jobs = self.jobs.lock().await;

        let job = match jobs.get_mut(&id) {
            Some(t) => t,
            None => return Err(JobQueueError::UnknownJob(id)),
        };

        if job.state != JobState::InProgress {
            tracing::error!("only in progress jobs are allowed to transition to other states");
            return Err(JobQueueError::Unknown);
        }

        match new_state {
            // this state should only exist when the job is first created
            JobState::New => {
                tracing::error!("can't transition an existing job to the New state");
                return Err(JobQueueError::Unknown);
            }
            // this is an internal transition that happens automatically when the job is picked up
            JobState::InProgress => {
                tracing::error!(
                    "only the job store may transition a job to the InProgress state"
                );
                return Err(JobQueueError::Unknown);
            }
            _ => (),
        }

        job.finished_at = Some(OffsetDateTime::now_utc());
        job.state = new_state;

        Ok(())
    }
}
