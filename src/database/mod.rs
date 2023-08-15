use crate::app::Config;

#[cfg(feature="postgres")]
mod postgres;

#[cfg(feature="sqlite")]
mod sqlite;

#[derive(Clone)]
pub enum Database {
    #[cfg(feature="postgres")]
    Postgres(PostgresDb),

    #[cfg(feature="sqlite")]
    Sqlite(SqliteDb),
}

#[axum::async_trait]
pub trait DbConn: Sized {
    type Database: sqlx::database::Database;

    async fn begin(&self) -> sqlx::Result<DbExecutor<Self::Database>>;
    async fn direct(&self) -> sqlx::Result<DbExecutor<Self::Database>>;

    async fn is_migrated(&self) -> Result<(), &str>;
    async fn run_migrations(&self) -> sqlx::Result<()>;
}

pub enum DbExecutor<'a, T: sqlx::database::Database> {
    Pool(sqlx::pool::Pool<T>),
    Transaction(sqlx::Transaction<'a, T>),
}

pub enum DbState {
    Setup,
    Migrating,
    Ready,
}

#[derive(Clone)]
pub struct PostgresDb {
    pool: sqlx::pool::Pool<sqlx::Postgres>,
    state: std::sync::Arc<tokio::sync::Mutex<DbState>>,
}

impl PostgresDb {
    fn new(pool: sqlx::pool::Pool<sqlx::Postgres>) -> Self {
        Self {
            pool,
            state: std::sync::Arc::new(tokio::sync::Mutex::new(DbState::Setup)),
        }
    }
}

#[derive(Clone)]
pub struct SqliteDb {
    pool: sqlx::pool::Pool<sqlx::Sqlite>,
    state: std::sync::Arc<tokio::sync::Mutex<DbState>>,
}

impl SqliteDb {
    fn new(pool: sqlx::pool::Pool<sqlx::Sqlite>) -> Self {
        Self {
            pool,
            state: std::sync::Arc::new(tokio::sync::Mutex::new(DbState::Setup)),
        }
    }
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
            let pool = postgres::configure_pool(db_url.as_str()).await?;
            Database::Postgres(PostgresDb::new(pool))
        }

        #[cfg(feature="sqlite")]
        db_url if db_url.starts_with("sqlite://") => {
            let pool = sqlite::configure_pool(db_url.as_str()).await?;
            Database::Sqlite(SqliteDb::new(pool))
        }

        _ => panic!("unknown database type, unable to setup database"),
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
