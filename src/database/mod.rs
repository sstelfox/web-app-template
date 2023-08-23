use crate::app::Config;

#[cfg(feature="postgres")]
mod postgres {
    use std::str::FromStr;

    use sqlx::migrate::Migrator;
    use sqlx::postgres::{PgConnectOptions, PgPool};

    use crate::database::DatabaseSetupError;

    static MIGRATOR: Migrator = sqlx::migrate!("migrations/postgres");

    pub(super) async fn configure_pool(url: &str) -> Result<PgPool, DatabaseSetupError> {
        let connection_options = PgConnectOptions::from_str(&url)
            .map_err(|err| DatabaseSetupError::BadUrl(err))?
            .application_name(env!("CARGO_PKG_NAME"))
            .statement_cache_capacity(250);

        let pool = sqlx::postgres::PgPoolOptions::new()
            .idle_timeout(std::time::Duration::from_secs(90))
            .max_lifetime(std::time::Duration::from_secs(1_800))
            .min_connections(1)
            .max_connections(16)
            .connect_lazy_with(connection_options);

        Ok(pool)
    }

    pub(super) async fn run_migrations(pool: &PgPool) -> Result<(), DatabaseSetupError> {
        MIGRATOR
            .run(pool)
            .await
            .map_err(|err| DatabaseSetupError::MigrationFailed(err))
    }
}

#[cfg(feature="sqlite")]
mod sqlite {
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

        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .idle_timeout(std::time::Duration::from_secs(90))
            .max_lifetime(std::time::Duration::from_secs(1_800))
            .min_connections(1)
            .max_connections(16)
            .connect_lazy_with(connection_options);

        Ok(pool)
    }

    pub(super) async fn run_migrations(pool: &SqlitePool) -> Result<(), DatabaseSetupError> {
        MIGRATOR
            .run(pool)
            .await
            .map_err(|err| DatabaseSetupError::MigrationFailed(err))
    }
}

#[axum::async_trait]
pub trait DbPool: Sized {
    type Database: sqlx::database::Database;

    async fn begin(&self) -> Result<DbExecutor<Self::Database>, DatabaseSetupError>;
    async fn direct(&self) -> Result<DbExecutor<Self::Database>, DatabaseSetupError>;

    async fn is_migrated(&self) -> bool;
    async fn run_migrations(&self) -> Result<(), DatabaseSetupError>;
}

#[derive(Clone)]
pub enum Db {
    #[cfg(feature="postgres")]
    Postgres(ProtectedDb<sqlx::Postgres>),

    #[cfg(feature="sqlite")]
    Sqlite(ProtectedDb<sqlx::Sqlite>),
}

impl Db {
    pub fn sql_flavor(&self) -> SqlFlavor {
        match self {
            #[cfg(feature="postgres")]
            Db::Postgres(_) => SqlFlavor::Postgres,

            #[cfg(feature="sqlite")]
            Db::Sqlite(_) => SqlFlavor::Sqlite,
        }
    }
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

pub struct ProtectedDb<T: sqlx::database::Database> {
    pool: sqlx::pool::Pool<T>,
    state: std::sync::Arc<tokio::sync::Mutex<DbState>>,
}

impl<T: sqlx::database::Database> Clone for ProtectedDb<T> {
    fn clone(&self) -> Self {
        Self {
            pool: self.pool.clone(),
            state: self.state.clone(),
        }
    }
}

impl<T: sqlx::database::Database> ProtectedDb<T> {
    fn new(pool: sqlx::pool::Pool<T>) -> Self {
        Self {
            pool,
            state: std::sync::Arc::new(tokio::sync::Mutex::new(DbState::Setup)),
        }
    }
}

#[cfg(feature="postgres")]
#[axum::async_trait]
impl DbPool for ProtectedDb<sqlx::Postgres> {
    type Database = sqlx::Postgres;

    async fn begin(&self) -> Result<DbExecutor<Self::Database>, DatabaseSetupError> {
        match &*self.state.lock().await {
            DbState::Setup => return Err(DatabaseSetupError::MigrationRequired),
            DbState::Migrating => return Err(DatabaseSetupError::MigrationInProgress),
            DbState::Ready => (),
        }

        let tx = self.pool.begin().await.map_err(|err| DatabaseSetupError::DatabaseUnavailable(err))?;
        Ok(DbExecutor::Transaction(tx))
    }

    async fn direct(&self) -> Result<DbExecutor<Self::Database>, DatabaseSetupError> {
        match &*self.state.lock().await {
            DbState::Setup => return Err(DatabaseSetupError::MigrationRequired),
            DbState::Migrating => return Err(DatabaseSetupError::MigrationInProgress),
            DbState::Ready => (),
        }

        Ok(DbExecutor::Pool(self.pool.clone()))
    }

    async fn is_migrated(&self) -> bool {
        let state = self.state.lock().await;
        matches!(&*state, DbState::Ready)
    }

    async fn run_migrations(&self) -> Result<(), DatabaseSetupError> {
        let mut state = self.state.lock().await;

        match &*state {
            DbState::Setup => (),
            DbState::Migrating => return Err(DatabaseSetupError::MigrationInProgress),
            DbState::Ready => return Ok(()),
        }

        *state = DbState::Migrating;
        drop(state);

        postgres::run_migrations(&self.pool).await?;

        let mut state = self.state.lock().await;
        *state = DbState::Ready;

        Ok(())
    }
}

#[cfg(feature="sqlite")]
#[axum::async_trait]
impl DbPool for ProtectedDb<sqlx::Sqlite> {
    type Database = sqlx::Sqlite;

    async fn begin(&self) -> Result<DbExecutor<Self::Database>, DatabaseSetupError> {
        match &*self.state.lock().await {
            DbState::Setup => return Err(DatabaseSetupError::MigrationRequired),
            DbState::Migrating => return Err(DatabaseSetupError::MigrationInProgress),
            DbState::Ready => (),
        }

        let tx = self.pool.begin().await.map_err(|err| DatabaseSetupError::DatabaseUnavailable(err))?;
        Ok(DbExecutor::Transaction(tx))
    }

    async fn direct(&self) -> Result<DbExecutor<Self::Database>, DatabaseSetupError> {
        match &*self.state.lock().await {
            DbState::Setup => return Err(DatabaseSetupError::MigrationRequired),
            DbState::Migrating => return Err(DatabaseSetupError::MigrationInProgress),
            DbState::Ready => (),
        }

        Ok(DbExecutor::Pool(self.pool.clone()))
    }

    async fn is_migrated(&self) -> bool {
        let state = self.state.lock().await;
        matches!(&*state, DbState::Ready)
    }

    async fn run_migrations(&self) -> Result<(), DatabaseSetupError> {
        let mut state = self.state.lock().await;

        match &*state {
            DbState::Setup => (),
            // todo: I could have a failed state, and include a counter on the migration attempt to
            // allow some background task to periodically retry
            DbState::Migrating => return Err(DatabaseSetupError::MigrationInProgress),
            DbState::Ready => return Ok(()),
        }

        *state = DbState::Migrating;
        drop(state);

        sqlite::run_migrations(&self.pool).await?;

        let mut state = self.state.lock().await;
        *state = DbState::Ready;

        Ok(())
    }
}

pub enum SqlFlavor {
    #[cfg(feature="postgres")]
    Postgres,

    #[cfg(feature="sqlite")]
    Sqlite,
}

pub async fn config_database(config: &Config) -> Result<Db, DatabaseSetupError> {
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
            Db::Postgres(ProtectedDb::new(pool))
        }

        #[cfg(feature="sqlite")]
        db_url if db_url.starts_with("sqlite://") => {
            let pool = sqlite::configure_pool(db_url.as_str()).await?;
            Db::Sqlite(ProtectedDb::new(pool))
        }

        _ => panic!("unknown database type, unable to setup database"),
    };

    Ok(db)
}

#[derive(Debug, thiserror::Error)]
pub enum DatabaseSetupError {
    #[error("provided database url wasn't valid")]
    BadUrl(sqlx::Error),

    #[error("failed to get a connection to the database")]
    DatabaseUnavailable(sqlx::Error),

    #[error("migrations are currently running on the database and must complete")]
    MigrationInProgress,

    #[error("unable to run pending migrations")]
    MigrationFailed(sqlx::migrate::MigrateError),

    #[error("migrations need to be run before a connection can be used")]
    MigrationRequired,
}