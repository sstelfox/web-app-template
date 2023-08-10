use std::str::FromStr;

use sqlx::migrate::Migrator;

use crate::app::Config;

#[cfg(feature="postgres")]
use sqlx::postgres::{PgConnectOptions, PgPool};

#[cfg(feature="postgres")]
static POSTGRES_MIGRATOR: Migrator = sqlx::migrate!("migrations/postgres");

#[cfg(feature="sqlite")]
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqliteSynchronous};

#[cfg(feature="sqlite")]
static SQLITE_MIGRATOR: Migrator = sqlx::migrate!("migrations/sqlite");

pub enum Database {
    #[cfg(feature="postgres")]
    Postgres(PgPool),

    #[cfg(feature="sqlite")]
    Sqlite(SqlitePool),
}

pub async fn config_database(config: &Config) -> Result<Database, DatabaseSetupError> {
    let database_url = match config.db_url() {
        Some(db_url) => db_url.to_string(),
        None => {
            match std::env::var("DATABASE_URL") {
                Ok(db_url) => db_url,
                Err(_) => "sqlite://data/database.db".to_string(),
            }
        }
    };

    // todo: I should figure out a way to delay the actual running of migrations, and reflect the
    // service being unavailable in the readiness check until they're complete
    //
    // maybe a tokio task with a channel or shared state directly that can be consumed by the
    // healthcheck and database extractor...

    match database_url {
        #[cfg(feature="postgres")]
        db_url if db_url.starts_with("postgres://") => {
            let connection_options = PgConnectOptions::from_str(&db_url)
                .map_err(|err| DatabaseSetupError::BadUrl(err))?
                .statement_cache_capacity(250);

            let pool = sqlx::PgPool::connect_with(connection_options)
                .await
                .map_err(|err| DatabaseSetupError::BadUrl(err))?;

            POSTGRES_MIGRATOR
                .run(&pool)
                .await
                .map_err(|err| DatabaseSetupError::MigrationFailed(err))?;

            return Ok(Database::Postgres(pool));
        }

        #[cfg(feature="sqlite")]
        db_url if db_url.starts_with("sqlite://") => {
            let connection_options = SqliteConnectOptions::from_str(&db_url)
                .map_err(|err| DatabaseSetupError::BadUrl(err))?
                .create_if_missing(true)
                .journal_mode(SqliteJournalMode::Wal)
                .statement_cache_capacity(250)
                .synchronous(SqliteSynchronous::Normal);

            let pool = sqlx::SqlitePool::connect_with(connection_options)
                .await
                .map_err(|err| DatabaseSetupError::DatabaseUnavailable(err))?;

            SQLITE_MIGRATOR
                .run(&pool)
                .await
                .map_err(|err| DatabaseSetupError::MigrationFailed(err))?;

            return Ok(Database::Sqlite(pool));
        }

        _ => panic!("invalid database url"),
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DatabaseSetupError {
    #[error("provided database url wasn't valid")]
    BadUrl(sqlx::Error),

    #[error("requested database wasn't available for initial connection testing")]
    DatabaseUnavailable(sqlx::Error),

    #[error("unable to run pending migrations")]
    MigrationFailed(sqlx::migrate::MigrateError),
}
