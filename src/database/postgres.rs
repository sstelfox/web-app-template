use std::str::FromStr;

use sqlx::migrate::Migrator;
use sqlx::postgres::{PgConnectOptions, PgPool};

use crate::database::DatabaseSetupError;

static MIGRATOR: Migrator = sqlx::migrate!("migrations/postgres");

pub(super) async fn configure_pool(url: &str) -> Result<PgPool, DatabaseSetupError> {
    let connection_options = PgConnectOptions::from_str(&url)
        .map_err(|err| DatabaseSetupError::BadUrl(err))?
        .statement_cache_capacity(250);

    let pool = sqlx::PgPool::connect_with(connection_options)
        .await
        .map_err(|err| DatabaseSetupError::BadUrl(err))?;

    run_migrations(&pool).await?;

    Ok(pool)
}

pub(super) async fn run_migrations(pool: &PgPool) -> Result<(), DatabaseSetupError> {
    MIGRATOR
        .run(pool)
        .await
        .map_err(|err| DatabaseSetupError::MigrationFailed(err))
}
