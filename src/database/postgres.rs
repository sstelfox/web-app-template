use std::str::FromStr;

use sqlx::migrate::Migrator;
use sqlx::postgres::{PgConnectOptions, PgPool};

use crate::database::DatabaseSetupError;

static MIGRATOR: Migrator = sqlx::migrate!("migrations/postgres");

#[derive(Clone)]
pub struct Executor {
    pool: PgPool,
}

pub(super) async fn create_executor(url: &str) -> Result<Executor, DatabaseSetupError> {
    let connection_options = PgConnectOptions::from_str(&url)
        .map_err(|err| DatabaseSetupError::BadUrl(err))?
        .statement_cache_capacity(250);

    let pool = sqlx::PgPool::connect_with(connection_options)
        .await
        .map_err(|err| DatabaseSetupError::BadUrl(err))?;

    MIGRATOR
        .run(&pool)
        .await
        .map_err(|err| DatabaseSetupError::MigrationFailed(err))?;

    Ok(Executor { pool })
}
