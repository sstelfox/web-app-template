#![allow(dead_code)]

use tasks::{MemoryTaskStore, TaskStore};

pub mod app;
mod database;
mod extractors;
mod health_check;
pub mod http_server;
mod tasks;

/// Sets up system panics to use the tracing infrastructure to log reported issues. This doesn't
/// prevent the panic from taking out the service but ensures that it and any available information
/// is properly reported using the standard logging mechanism.
pub fn register_panic_logger() {
    std::panic::set_hook(Box::new(|panic| match panic.location() {
        Some(loc) => {
            tracing::error!(
                message = %panic,
                panic.file = loc.file(),
                panic.line = loc.line(),
                panic.column = loc.column(),
            );
        }
        None => tracing::error!(message = %panic),
    }));
}

pub fn report_version() {
    let version = app::Version::new();
    tracing::info!(
        build_profile = ?version.build_profile,
        features = ?version.features,
        version = ?version.version,
        "service starting up"
    );
}

pub async fn test_tasks_placeholder() {
    // playing around with the background task system, this is not the final API
    let mut mts = MemoryTaskStore::default();

    for num in [78, 23, 102].iter() {
        let id = MemoryTaskStore::enqueue(&mut mts, tasks::TestTask::new(*num))
            .await
            .unwrap();
        tracing::info!(?id, "enqueued task");
    }

    while let Some(task) = mts.next("default").await.unwrap() {
        tracing::info!(id = ?task.id, "running task");
        mts.update_state(task.id, tasks::TaskState::Complete)
            .await
            .unwrap();
    }
}

#[cfg(test)]
mod tests;
