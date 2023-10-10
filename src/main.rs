use std::time::Duration;

use futures::future::join_all;
use tokio::time::timeout;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

use web_app_template::app::Config;

const FINAL_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(30);

#[tokio::main]
async fn main() {
    let config = match Config::from_env_and_args() {
        Ok(c) => c,
        Err(err) => {
            println!("failed to load config: {err}");
            std::process::exit(2);
        }
    };

    let (non_blocking_writer, _guard) = tracing_appender::non_blocking(std::io::stdout());
    let env_filter = EnvFilter::builder()
        .with_default_directive(config.log_level().into())
        .from_env_lossy();

    let stderr_layer = tracing_subscriber::fmt::layer()
        .compact()
        .with_writer(non_blocking_writer)
        .with_filter(env_filter);

    tracing_subscriber::registry().with(stderr_layer).init();

    web_app_template::register_panic_logger();
    web_app_template::report_version();

    let (graceful_waiter, shutdown_rx) = web_app_template::graceful_shutdown_blocker();
    let (worker_handle, mut work_scheduler) = web_app_template::background_workers(shutdown_rx.clone()).await;
    // todo: pass work scheduler into http server for its state
    let http_handle = web_app_template::http_server(config, shutdown_rx.clone()).await;

    for num in [78, 23, 102].iter() {
        work_scheduler.enqueue(web_app_template::tasks::TestTask::new(*num))
            .await
            .expect("enqueue to succeed");
    }

    let _ = graceful_waiter.await;

    if let Err(_) = timeout(FINAL_SHUTDOWN_TIMEOUT, join_all(vec![worker_handle, http_handle])).await {
        tracing::error!("hit final shutdown timeout. exiting with remaining work in progress");
        std::process::exit(3);
    }
}

#[derive(Debug, thiserror::Error)]
enum ServiceError {
    #[error("service couldn't initialize the config: {0}")]
    ConfigSetupFailed(#[from] web_app_template::app::ConfigError),

    #[error("service encountered an issue: {0}")]
    RunFailed(#[from] web_app_template::http_server::HttpServerError),
}
