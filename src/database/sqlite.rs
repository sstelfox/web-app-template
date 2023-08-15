use std::str::FromStr;

use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqliteSynchronous};

use crate::database::DatabaseSetupError;

static MIGRATOR: Migrator = sqlx::migrate!("migrations/sqlite");

pub(super) async fn configure_pool(url: &str) -> Result<SqlitePool, DatabaseSetupError> {
    let connection_options = SqliteConnectOptions::from_str(url)
        .map_err(|err| DatabaseSetupError::BadUrl(err))?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .statement_cache_capacity(250)
        .synchronous(SqliteSynchronous::Normal);

    let pool = sqlx::SqlitePool::connect_with(connection_options)
        .await
        .map_err(|err| DatabaseSetupError::DatabaseUnavailable(err))?;

    run_migrations(&pool).await?;

    Ok(pool)
}

pub(super) async fn run_migrations(pool: &SqlitePool) -> Result<(), DatabaseSetupError> {
    MIGRATOR
        .run(pool)
        .await
        .map_err(|err| DatabaseSetupError::MigrationFailed(err))
}
