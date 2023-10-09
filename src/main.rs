use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

use web_app_template::app::Config;

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
    web_app_template::test_tasks_placeholder().await;

    match web_app_template::http_server::run(config).await {
        Ok(_) => tracing::info!("shutting down normally"),
        Err(err) => tracing::error!("http server exited with an error: {err}"),
    }
}

#[derive(Debug, thiserror::Error)]
enum ServiceError {
    #[error("service couldn't initialize the config: {0}")]
    ConfigSetupFailed(#[from] web_app_template::app::ConfigError),

    #[error("service encountered an issue: {0}")]
    RunFailed(#[from] web_app_template::http_server::HttpServerError),
}
