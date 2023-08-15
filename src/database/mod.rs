use crate::app::Config;

#[cfg(feature="postgres")]
mod postgres;

#[cfg(feature="sqlite")]
mod sqlite;

#[derive(Clone)]
pub enum Database {
    #[cfg(feature="postgres")]
    Postgres(postgres::Executor),

    #[cfg(feature="sqlite")]
    Sqlite(sqlite::Executor),
}

pub async fn config_database(config: &Config) -> Result<Database, DatabaseSetupError> {
    let database_url = match config.db_url() {
        Some(db_url) => db_url.to_string(),
        None => {
            match std::env::var("DATABASE_URL") {
                Ok(db_url) => db_url,
                Err(_) => "sqlite://:memory:".to_string(),
            }
        }
    };

    // todo: I should figure out a way to delay the actual connection and running of migrations,
    // and reflect the service being unavailable in the readiness check until they're complete. If
    // our connection fails we should try a couple of times with a backoff before failing the
    // entire service...
    //
    // maybe a tokio task with a channel or shared state directly that can be consumed by the
    // healthcheck and database extractor... Maybe this state belongs on the database executor
    // itself...

    let db = match database_url {
        #[cfg(feature="postgres")]
        db_url if db_url.starts_with("postgres://") => {
            let executor = postgres::create_executor(db_url.as_str()).await?;
            Database::Postgres(executor)
        }

        #[cfg(feature="sqlite")]
        db_url if db_url.starts_with("sqlite://") => {
            let executor = sqlite::create_executor(db_url.as_str()).await?;
            Database::Sqlite(executor)
        }

        _ => panic!("invalid database url"),
    };

    Ok(db)
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
