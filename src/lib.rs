#![allow(dead_code)]

use std::time::Duration;

use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::watch;
use tokio::task::JoinHandle;

pub mod app;
mod auth;
mod database;
mod extractors;
mod health_check;
pub mod http_server;
pub mod jobs;
pub mod utils;

const REQUEST_GRACE_PERIOD: Duration = Duration::from_secs(10);

pub async fn background_workers(mut shutdown_rx: watch::Receiver<()>) -> JoinHandle<()> {
    let mts = jobs::MemoryTaskStore::default();

    jobs::WorkerPool::new(mts, move || { () })
        .register_task_type::<jobs::TestTask>()
        .configure_queue(jobs::QueueConfig::new("default"))
        .start(async move { let _ = shutdown_rx.changed().await; })
        .await
        .expect("worker start up to succeed")
}

/// Follow k8s signal handling rules for these different signals. The order of shutdown events are:
///
/// 1. Pod is set to the "Terminating" state and removed from the endpoints list of all services,
///    new traffic should stop appearing
/// 2. The preStop Hook is executed if configured, can send a command or an http request. Should be
///    implemented if SIGTERM doesn't gracefully shutdown your app. Simultaneously k8s will start
///    issuing endpoint update commands indicating the service should be removed from load
///    balancers.
/// 3. SIGTERM signal is sent to the pod, your service should start shutting down cleanly, service
///    has 30 seconds to perform any clean up, shutdown, and state saving. The service may still
///    receive requests for up to 10 seconds on GKE according to some blog post. This would make
///    sense as the event time needs to propagate through the system and is supported by this quote
///    about service meshes:
///
///    > Since the components might be busy doing something else, there is no guarantee on how
///    > long it will take to remove the IP address from their internal state.
///
///    I've seen recommendations that the readiness probe should start failing here and others
///    reporting that won't do anything. As far as I can tell failing the readiness probe here
///    makes sense and at worse will do nothing.
///
///    It seems that the common recommendation here is to wait for 10-15 seconds in the
///    graceperiod, with readiness failing, then exit
/// 4. If the container doesn't exit on its own after 30 seconds it will receive a SIGKILL which we
///    can't respond to, we just get killed.
///
/// This also handles SIGINT which K8s doesn't issue, those will be coming from users running the
/// server locally and should shut the server down immediately.
pub fn graceful_shutdown_blocker() -> (JoinHandle<()>, watch::Receiver<()>) {
    let mut sigint = signal(SignalKind::interrupt()).unwrap();
    let mut sigterm = signal(SignalKind::terminate()).unwrap();

    let (tx, rx) = tokio::sync::watch::channel(());

    let handle = tokio::spawn(async move {
        tokio::select! {
            _ = sigint.recv() => {
                tracing::debug!("gracefully exiting immediately on SIGINT");
            }
            _ = sigterm.recv() => tracing::debug!("initiaing graceful shutdown with delay on SIGTERM"),
        }

        // todo: this is the desired k8s behavior... but for our current usage, we don't have
        // layers of proxies that require information progagation. This just increases the errors
        // visible during deploys
        //tokio::time::sleep(REQUEST_GRACE_PERIOD).await

        // Time to start signaling any services that care about gracefully shutting down that the
        // time is at hand.
        let _ = tx.send(());

        // todo: fail the readiness checks (should probably be handled by something with a copy of
        // the receiver...
    });

    (handle, rx)
}

pub async fn http_server(config: app::Config, shutdown_rx: watch::Receiver<()>) -> JoinHandle<()> {
    tokio::spawn(async move {
        match http_server::run(config, shutdown_rx).await {
            Ok(_) => tracing::info!("shutting down normally"),
            Err(err) => tracing::error!("http server exited with an error: {err}"),
        }
    })
}

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

#[cfg(test)]
mod tests;
