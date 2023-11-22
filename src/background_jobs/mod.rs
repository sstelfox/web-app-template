#![allow(dead_code)]

mod catch_panic_future;
pub mod impls;
mod interface;
mod queue_config;
mod stores;
mod worker;
mod worker_pool;

use catch_panic_future::{CatchPanicFuture, CaughtPanic};
pub use queue_config::QueueConfig;
pub use stores::basic_task_store::{BasicTaskContext, BasicTaskStore};
pub use stores::event_task_store::{EventTaskContext, EventTaskStore};
pub use stores::JobStoreError;
use stores::{ExecuteJobFn, JobExecError, JobStore, StateFn};
use worker::Worker;
pub use worker_pool::WorkerPool;

use std::time::Duration;

use axum::async_trait;
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::database::custom_types::{BackgroundJobId, BackgroundRunId, UniqueTaskKey};
use crate::database::models::BackgroundJob;

const JOB_EXECUTION_TIMEOUT: Duration = Duration::from_secs(30);

const MAXIMUM_CHECK_DELAY: Duration = Duration::from_millis(250);

#[async_trait]
pub trait JobLike: Serialize + DeserializeOwned + Sync + Send + 'static {
    const JOB_NAME: &'static str;

    const MAX_ATTEMPTS: u8 = 3;

    const QUEUE_NAME: &'static str = "default";

    type Context: Clone + Send + 'static;
    type Error: std::error::Error;

    async fn run(&self, ctx: Self::Context) -> Result<(), Self::Error>;

    async fn unique_key(&self) -> Option<UniqueTaskKey> {
        None
    }
}

#[async_trait]
pub trait JobLikeExt {
    async fn enqueue<S: JobStore>(
        self,
        connection: &mut S::Connection,
    ) -> Result<Option<(BackgroundJobId, BackgroundRunId)>, JobStoreError>;
}

#[async_trait]
impl<J> JobLikeExt for J
where
    J: JobLike,
{
    async fn enqueue<S: JobStore>(
        self,
        connection: &mut S::Connection,
    ) -> Result<Option<(BackgroundJobId, BackgroundRunId)>, JobStoreError> {
        S::enqueue(connection, self).await
    }
}

//fn sort_jobs(a: &BackgroundJob, b: &BackgroundJob) -> Ordering {
//    match a.attempt_run_at.cmp(&b.attempt_run_at) {
//        Ordering::Equal => a.scheduled_at.cmp(&b.scheduled_at),
//        ord => ord,
//    }
//}

//#[derive(Clone, Default)]
//pub struct MemoryJobStore {
//    pub jobs: Arc<Mutex<BTreeMap<BackgroundJobId, Job>>>,
//}
//
//impl MemoryJobStore {
//    // note: might want to extend this to be unique over a queue... I could just prepend the queue
//    // the key or something...
//    async fn is_key_present(conn: &Self, key: &str) -> bool {
//        let jobs = conn.jobs.lock().await;
//
//        for (_, job) in jobs.iter() {
//            // we only need to look at a job if it also has a unique key
//            let existing_key = match &job.unique_key {
//                Some(ek) => ek,
//                None => continue,
//            };
//
//            // any job that has already ended isn't considered for uniqueness checks
//            if !matches!(
//                job.state,
//                BackgroundJobState::New | BackgroundJobState::InProgress | BackgroundJobState::Retry
//            ) {
//                continue;
//            }
//
//            // we found a match, we don't need to enqueue a new job
//            if key == existing_key {
//                return true;
//            }
//        }
//
//        false
//    }
//}
//
//#[async_trait]
//impl JobStore for MemoryJobStore {
//    type Connection = Self;
//
//    async fn enqueue<T: JobLike>(
//        conn: &mut Self::Connection,
//        job: T,
//    ) -> Result<Option<BackgroundJobId>, JobStoreError> {
//        let unique_key = job.unique_key().await;
//
//        if let Some(new_key) = &unique_key {
//            if MemoryJobStore::is_key_present(conn, new_key).await {
//                return Ok(None);
//            }
//        }
//
//        let id = BackgroundJobId::from(Uuid::new_v4());
//        let payload = serde_json::to_value(job).map_err(|_| JobStoreError::Unknown)?;
//
//        let job = Job {
//            id,
//
//            name: T::JOB_NAME.to_string(),
//            queue_name: T::QUEUE_NAME.to_string(),
//
//            unique_key,
//            state: BackgroundJobState::New,
//            current_attempt: 0,
//            maximum_attempts: T::MAX_RETRIES,
//
//            payload: Some(payload),
//
//            scheduled_at: OffsetDateTime::now_utc(),
//            attempt_run_at: OffsetDateTime::now_utc(),
//        };
//
//        let mut jobs = conn.jobs.lock().await;
//        jobs.insert(job.id, job);
//
//        Ok(Some(id))
//    }
//
//    async fn next(
//        &self,
//        queue_name: &str,
//        job_names: &[&str],
//    ) -> Result<Option<Job>, JobStoreError> {
//        let mut jobs = self.jobs.lock().await;
//        let mut next_job = None;
//
//        let reference_time = OffsetDateTime::now_utc();
//        let mut jobs_to_retry = Vec::new();
//
//        for (id, job) in jobs
//            .iter_mut()
//            .filter(|(_, job)| {
//                job_names.contains(&job.name.as_str()) && job.attempt_run_at <= reference_time
//            })
//            // only care about jobs that have a state to advance
//            .filter(|(_, job)| {
//                matches!(
//                    job.state,
//                    BackgroundJobState::New | BackgroundJobState::InProgress | BackgroundJobState::Retry
//                )
//            })
//            .sorted_by(|a, b| sort_jobs(a.1, b.1))
//        {
//            match (job.state, job.started_at) {
//                (BackgroundJobState::New | BackgroundJobState::Retry, None) => {
//                    if job.queue_name != queue_name {
//                        continue;
//                    }
//
//                    job.started_at = Some(OffsetDateTime::now_utc());
//                    job.state = BackgroundJobState::InProgress;
//
//                    next_job = Some(job.clone());
//                    break;
//                }
//                (BackgroundJobState::InProgress, Some(started_at)) => {
//                    if (started_at + JOB_EXECUTION_TIMEOUT) >= OffsetDateTime::now_utc() {
//                        // todo: need to send cancel signal to the job
//                        job.state = BackgroundJobState::TimedOut;
//                        job.finished_at = Some(OffsetDateTime::now_utc());
//
//                        jobs_to_retry.push(id);
//                    }
//                }
//                (state, _) => {
//                    tracing::error!(id = ?job.id, ?state, "encountered job in illegal state");
//                    job.state = BackgroundJobState::Dead;
//                    job.finished_at = Some(OffsetDateTime::now_utc());
//                }
//            }
//        }
//
//        for id in jobs_to_retry.into_iter() {
//            // attempt to requeue any of these jobs we encountered, if we fail to requeue them its
//            // not a big deal but we will keep trying if they stay in that state... Might want to
//            // put some kind of time window on these or something
//            let _ = self.retry(*id).await;
//        }
//
//        Ok(next_job)
//    }
//
//    async fn retry(&self, id: BackgroundJobId) -> Result<Option<BackgroundJobId>, JobStoreError> {
//        let mut jobs = self.jobs.lock().await;
//
//        let target_job = match jobs.get_mut(&id) {
//            Some(t) => t,
//            None => return Err(JobStoreError::UnknownJob(id)),
//        };
//
//        // these states are the only retryable states
//        if !matches!(target_job.state, BackgroundJobState::Error | BackgroundJobState::TimedOut) {
//            tracing::warn!(?id, "job is not in a state that can be retried");
//            return Err(JobStoreError::Unknown);
//        }
//
//        // no retries remaining mark the job as dead
//        if target_job.current_attempt >= target_job.maximum_attempts {
//            tracing::warn!(?id, "job failed with no more attempts remaining");
//            target_job.state = BackgroundJobState::Dead;
//            return Ok(None);
//        }
//
//        let mut new_job = target_job.clone();
//
//        let new_id = BackgroundJobId::from(Uuid::new_v4());
//        target_job.next_id = Some(new_job.id);
//
//        new_job.id = new_id;
//        new_job.previous_id = Some(target_job.id);
//
//        new_job.current_attempt += 1;
//        new_job.state = BackgroundJobState::Retry;
//        new_job.started_at = None;
//        new_job.scheduled_at = OffsetDateTime::now_utc();
//
//        // really rough exponential backoff, 4, 8, and 16 seconds by default
//        let backoff_secs = 2u64.saturating_pow(new_job.current_attempt.saturating_add(1) as u32);
//        tracing::info!(
//            ?id,
//            "job will be retried {backoff_secs} secs in the future"
//        );
//        new_job.attempt_run_at = OffsetDateTime::now_utc() + Duration::from_secs(backoff_secs);
//
//        jobs.insert(new_job.id, new_job);
//
//        Ok(Some(new_id))
//    }
//
//    async fn update_state(&self, id: BackgroundJobId, new_state: BackgroundJobState) -> Result<(), JobStoreError> {
//        let mut jobs = self.jobs.lock().await;
//
//        let job = match jobs.get_mut(&id) {
//            Some(t) => t,
//            None => return Err(JobStoreError::UnknownJob(id)),
//        };
//
//        if job.state != BackgroundJobState::InProgress {
//            tracing::error!("only in progress jobs are allowed to transition to other states");
//            return Err(JobStoreError::Unknown);
//        }
//
//        match new_state {
//            // this state should only exist when the job is first created
//            BackgroundJobState::New => {
//                tracing::error!("can't transition an existing job to the New state");
//                return Err(JobStoreError::Unknown);
//            }
//            // this is an internal transition that happens automatically when the job is picked up
//            BackgroundJobState::InProgress => {
//                tracing::error!("only the job store may transition a job to the InProgress state");
//                return Err(JobStoreError::Unknown);
//            }
//            _ => (),
//        }
//
//        job.finished_at = Some(OffsetDateTime::now_utc());
//        job.state = new_state;
//
//        Ok(())
//    }
//}
