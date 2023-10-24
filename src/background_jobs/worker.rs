use std::collections::BTreeMap;

use tokio::sync::watch::Receiver;

use crate::background_jobs::{MAXIMUM_CHECK_DELAY, CatchPanicFuture, ExecuteJobFn, BackgroundJob, JobQueueError, JobStore, QueueConfig, StateFn};

pub struct Worker<Context, S>
where
    Context: Clone + Send + 'static,
    S: JobStore + Clone,
{
    name: String,
    queue_config: QueueConfig,

    context_data_fn: StateFn<Context>,
    store: S,
    job_registry: BTreeMap<&'static str, ExecuteJobFn<Context>>,

    shutdown_signal: Option<Receiver<()>>,
}

impl<Context, S> Worker<Context, S>
where
    Context: Clone + Send + 'static,
    S: JobStore + Clone,
{
    pub fn new(
        name: String,
        queue_config: QueueConfig,
        context_data_fn: StateFn<Context>,
        store: S,
        job_registry: BTreeMap<&'static str, ExecuteJobFn<Context>>,
        shutdown_signal: Option<Receiver<()>>,
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

    async fn run(&self, job: BackgroundJob) -> Result<(), WorkerError> {
        let deserialize_and_run_job_fn = self
            .job_registry
            .get(job.name.as_str())
            .ok_or(WorkerError::UnregisteredJobName(job.name))?
            .clone();

        // create a new JobRun for the job

        let payload = job.payload.ok_or(WorkerError::PayloadMissing)?.clone();
        let safe_runner = CatchPanicFuture::wrap({
            let context = (self.context_data_fn)();
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
                //self.store
                //    .update_state(job.id, BackgroundJobState::Panicked)
                //    .await
                //    .map_err(WorkerError::UpdateJobStatusFailed)?;

                // we didn't complete successfully, but we do want to keep processing jobs for
                // now. We may be corrupted due to the panic somehow if additional errors crop up.
                // Left as future work to handle this edge case.
                return Ok(());
            }
        };

        //match job_result {
        //    Ok(_) => {
        //        self.store
        //            .update_state(job.id, BackgroundJobState::Complete)
        //            .await
        //            .map_err(WorkerError::UpdateJobStatusFailed)?;
        //    }
        //    Err(err) => {
        //        tracing::error!("job failed with error: {err}");

        //        self.store
        //            .update_state(job.id, BackgroundJobState::Error)
        //            .await
        //            .map_err(WorkerError::UpdateJobStatusFailed)?;

        //        self.store
        //            .retry(job.id)
        //            .await
        //            .map_err(WorkerError::RetryJobFailed)?;
        //    }
        //}

        Ok(())
    }

    pub async fn run_jobs(&mut self) -> Result<(), WorkerError> {
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

    #[error("attempted to run job that already had its payload cleared")]
    PayloadMissing,

    #[error("failed to enqueue a failed job for re-execution: {0}")]
    RetryJobFailed(JobQueueError),

    #[error("error while attempting to retrieve the next job: {0}")]
    StoreUnavailable(JobQueueError),

    #[error("failed to update job status with store: {0}")]
    UpdateJobStatusFailed(JobQueueError),

    #[error("during execution of a dequeued job, encountered unregistered job '{0}'")]
    UnregisteredJobName(String),
}
