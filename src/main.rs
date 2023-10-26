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
    use web_app_template::llm::hugging_face;

    let vers = hugging_face::check_safetensor_model_version(hugging_face::EMBEDDING_MODEL)
        .await
        .expect("valid");
    println!("{:?}", vers);

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

    let state = match web_app_template::app::State::from_config(&config).await {
        Ok(s) => s,
        Err(err) => {
            tracing::error!("failed to initialize state: {err}");
            std::process::exit(3);
        }
    };

    let (graceful_waiter, shutdown_rx) = web_app_template::graceful_shutdown_blocker();

    let mut all_handles = Vec::new();

    let worker_handles =
        web_app_template::background_workers(state.clone(), shutdown_rx.clone()).await;
    all_handles.extend(worker_handles);

    let http_handle = web_app_template::http_server(
        *config.listen_addr(),
        config.log_level(),
        state,
        shutdown_rx.clone(),
    )
    .await;
    all_handles.push(http_handle);

    let _ = graceful_waiter.await;

    if (timeout(FINAL_SHUTDOWN_TIMEOUT, join_all(all_handles)).await).is_err() {
        tracing::error!("hit final shutdown timeout. exiting with remaining work in progress");
        std::process::exit(4);
    }
}

#[derive(Debug, thiserror::Error)]
enum ServiceError {
    #[error("service couldn't initialize the config: {0}")]
    ConfigSetupFailed(#[from] web_app_template::app::ConfigError),

    #[error("service encountered an issue: {0}")]
    RunFailed(#[from] web_app_template::http_server::HttpServerError),
}
