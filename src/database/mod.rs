use axum::extract::FromRef;
use sqlx::SqlitePool;

use crate::app::State;

pub type Database = SqlitePool;

pub async fn connect(db_url: &url::Url) -> DbResult<Database> {
    // todo: I should figure out a way to delay the actual connection and running of migrations,
    // and reflect the service being unavailable in the readiness check until they're complete. If
    // our connection fails we should try a couple of times with a backoff before failing the
    // entire service...
    //
    // maybe a tokio task with a channel or shared state directly that can be consumed by the
    // healthcheck and database extractor... Maybe this state belongs on the database executor
    // itself...

    if db_url.scheme() == "sqlite" {
        let db = sqlite::connect_sqlite(db_url).await?;
        sqlite::mitrate_sqlite(&db).await?;
        return Ok(db);
    }

    Err(DatabaseError::UnknownDbType)
}

#[derive(Debug, thiserror::Error)]
pub enum DatabaseError {
    #[error("unable to load data from database, appears to be invalid")]
    CorruptData(sqlx::Error),

    #[error("unable to communicate with the database")]
    DatabaseUnavailable(sqlx::Error),

    #[error("an internal database error occurred")]
    InternalError(sqlx::Error),

    #[error("error occurred while attempting database migration")]
    MigrationFailed(sqlx::migrate::MigrateError),

    #[error("unable to create record as it would violate a uniqueness constraint")]
    RecordExists,

    #[error("unable to locate record or associated foreign key")]
    RecordNotFound,

    #[error("requested database type was not recognized")]
    UnknownDbType,
}

pub type DbResult<T = ()> = Result<T, DatabaseError>;

pub mod sqlite {
    use std::time::Duration;

    use sqlx::migrate::Migrator;
    use sqlx::sqlite::{
        SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions, SqliteSynchronous,
    };
    use sqlx::ConnectOptions;
    use url::Url;

    use super::{DatabaseError, DbResult};

    static MIGRATOR: Migrator = sqlx::migrate!();

    pub async fn connect_sqlite(url: &Url) -> DbResult<SqlitePool> {
        let connection_options = SqliteConnectOptions::from_url(url)
            .map_err(|err| DatabaseError::DatabaseUnavailable(err))?
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .statement_cache_capacity(250)
            .synchronous(SqliteSynchronous::Normal);

        SqlitePoolOptions::new()
            .idle_timeout(Duration::from_secs(90))
            .max_lifetime(Duration::from_secs(1_800))
            .min_connections(1)
            .max_connections(16)
            .connect_with(connection_options)
            .await
            .map_err(|err| DatabaseError::DatabaseUnavailable(err))
    }

    pub async fn mitrate_sqlite(pool: &SqlitePool) -> DbResult {
        MIGRATOR
            .run(pool)
            .await
            .map_err(|err| DatabaseError::MigrationFailed(err))
    }
}
