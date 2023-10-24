use async_trait::async_trait;
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::background_jobs::JobLike;

#[derive(Deserialize, Serialize)]
pub struct TestJob<C: Clone + Send + Sync + 'static> {
    number: usize,
    _phantom: std::marker::PhantomData<C>,
}

impl<C: Clone + Send + Sync + 'static> TestJob<C> {
    pub fn new(number: usize) -> Self {
        Self {
            number,
            _phantom: std::marker::PhantomData,
        }
    }
}

#[async_trait]
impl<C: Clone + Send + Sync + 'static> JobLike for TestJob<C> {
    const JOB_NAME: &'static str = "test_job";

    type Error = TestJobError;
    type Context = C;

    async fn run(&self, _ctx: Self::Context) -> Result<(), Self::Error> {
        let mut rng = rand::thread_rng();

        if rng.gen_bool(0.1) {
            return Err(TestJobError::RandomFailure);
        }

        tracing::info!("the test task value is {}", self.number);

        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TestJobError {
    #[error("the test job failed its randomness check")]
    RandomFailure,
}
