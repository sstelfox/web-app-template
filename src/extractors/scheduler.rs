use std::convert::Infallible;
use std::ops::DerefMut;

use axum::async_trait;
use axum::extract::{FromRef, FromRequestParts};
use http::request::Parts;

use crate::tasks::{MemoryTaskStore, TaskId, TaskLike, TaskLikeExt, TaskQueueError, WorkScheduler};

pub struct Scheduler(WorkScheduler<MemoryTaskStore>);

impl Scheduler {
    pub async fn enqueue(&mut self, task: impl TaskLike) -> Result<Option<TaskId>, TaskQueueError> {
        task.enqueue::<MemoryTaskStore>(self.0.deref_mut())
            .await
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for Scheduler
where
    WorkScheduler<MemoryTaskStore>: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(_parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        Ok(Scheduler(WorkScheduler::from_ref(state)))
    }
}
