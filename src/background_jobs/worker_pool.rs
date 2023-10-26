use std::collections::BTreeMap;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use futures::future::join_all;
use futures::Future;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio::time::timeout;

use crate::background_jobs::{
    ExecuteJobFn, JobExecError, JobLike, JobStore, QueueConfig, StateFn, Worker,
};

const WORKER_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

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
                    if let Err(err) = worker.run_jobs().await {
                        tracing::error!(name = ?worker_name, "worker stopped due to error: {err}")
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

#[derive(Debug, thiserror::Error)]
pub enum WorkerPoolError {
    #[error("found queue '{0}' defined by job(s) {1:?} without a queue config")]
    QueueNotConfigured(&'static str, Vec<&'static str>),
}

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
            // todo: should try and serialize the error if possible
            Err(run_err) => Err(JobExecError::ExecutionFailed(run_err.to_string())),
        }
    })
}
