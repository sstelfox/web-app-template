use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

use web_app_template::app::{Config, Error};

#[tokio::main]
async fn main() -> Result<(), Error> {
    let config = Config::from_env_and_args()?;

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

    web_app_template::http_server::run(config).await?;

    tracing::info!("shutting down normally");

    Ok(())
}
