use std::sync::Arc;

use axum::async_trait;

//#[cfg(all(feature = "postgres", feature = "sqlite"))]
//compile_error!("Database selection features `postgres` and `sqlite` are mutually exclusive, you cannot enable both!");

#[cfg(not(any(feature = "postgres", feature = "sqlite")))]
compile_error!("You must enable at least one database features: `postgres` or `sqlite`");

pub type Database = Arc<dyn Db + Send + Sync>;

pub async fn connect(db_url: &str) -> DbResult<Database> {
    // todo: I should figure out a way to delay the actual connection and running of migrations,
    // and reflect the service being unavailable in the readiness check until they're complete. If
    // our connection fails we should try a couple of times with a backoff before failing the
    // entire service...
    //
    // maybe a tokio task with a channel or shared state directly that can be consumed by the
    // healthcheck and database extractor... Maybe this state belongs on the database executor
    // itself...

    #[cfg(feature="postgres")]
    if db_url.starts_with("postgres://") {
        let db = postgres::PostgresDb::connect(db_url).await?;
        db.migrate().await?;
        return Ok(Arc::new(db));
    }

    #[cfg(feature="sqlite")]
    if db_url.starts_with("sqlite://") {
        let db = sqlite::SqliteDb::connect(db_url).await?;
        db.migrate().await?;
        return Ok(Arc::new(db));
    }

    panic!("unknown database type, unable to setup database");
}

#[async_trait]
pub trait Db {
    fn ex(&self) -> Executor;

    async fn begin(&self) -> DbResult<TxExecutor>;
}

#[derive(Debug, thiserror::Error)]
pub enum DbError {
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
}

pub type DbResult<T = ()> = Result<T, DbError>;

pub enum Executor {
    #[cfg(feature="postgres")]
    Postgres(postgres::PostgresExecutor),

    #[cfg(feature="sqlite")]
    Sqlite(sqlite::SqliteExecutor),
}

pub struct TxExecutor(Executor);

impl TxExecutor {
    pub fn ex(&mut self) -> &mut Executor {
        &mut self.0
    }

    pub async fn commit(self) -> DbResult {
        match self.0 {
            #[cfg(feature="postgres")]
            Executor::Postgres(e) => e.commit().await,

            #[cfg(feature="sqlite")]
            Executor::Sqlite(e) => e.commit().await,
        }
    }
}

#[cfg(feature="postgres")]
pub mod postgres {
    use std::str::FromStr;

    use axum::async_trait;
    use futures::future::BoxFuture;
    use sqlx::Transaction;
    use sqlx::migrate::Migrator;
    use sqlx::postgres::{PgConnectOptions, PgDatabaseError, PgPool, Postgres};

    use super::{Db, DbError, DbResult, Executor, TxExecutor};

    static MIGRATOR: Migrator = sqlx::migrate!("migrations/postgres");

    #[derive(Debug)]
    pub enum PostgresExecutor {
        PoolExec(PgPool),
        TxExec(Transaction<'static, Postgres>),
    }

    impl PostgresExecutor {
        pub async fn commit(self) -> DbResult {
            match self {
                Self::PoolExec(_) => unreachable!("need to check this, but it shouldn't be called"),
                Self::TxExec(tx) => tx.commit().await.map_err(map_sqlx_error),
            }
        }
    }

    impl<'c> sqlx::Executor<'c> for &'c mut PostgresExecutor {
        type Database = Postgres;

        fn describe<'e, 'q: 'e>(
            self,
            sql: &'q str,
        ) -> BoxFuture<'e, Result<sqlx::Describe<Self::Database>, sqlx::Error>>
        where
            'c: 'e,
        {
            match self {
                PostgresExecutor::PoolExec(pool) => pool.describe(sql),
                PostgresExecutor::TxExec(ref mut tx) => tx.describe(sql),
            }
        }

        fn fetch_many<'e, 'q: 'e, E: 'q>(
            self,
            query: E,
        ) -> futures::stream::BoxStream<
            'e,
            Result<
                sqlx::Either<
                    <Self::Database as sqlx::Database>::QueryResult,
                    <Self::Database as sqlx::Database>::Row,
                >,
                sqlx::Error,
            >,
        >
        where
            'c: 'e,
            E: sqlx::Execute<'q, Self::Database>,
        {
            match self {
                PostgresExecutor::PoolExec(pool) => pool.fetch_many(query),
                PostgresExecutor::TxExec(ref mut tx) => tx.fetch_many(query),
            }
        }

        fn fetch_optional<'e, 'q: 'e, E: 'q>(
            self,
            query: E,
        ) -> BoxFuture<'e, Result<Option<<Self::Database as sqlx::Database>::Row>, sqlx::Error>>
        where
            'c: 'e,
            E: sqlx::Execute<'q, Self::Database>,
        {
            match self {
                PostgresExecutor::PoolExec(pool) => pool.fetch_optional(query),
                PostgresExecutor::TxExec(ref mut tx) => tx.fetch_optional(query),
            }
        }

        fn prepare_with<'e, 'q: 'e>(
            self,
            sql: &'q str,
            parameters: &'e [<Self::Database as sqlx::Database>::TypeInfo],
        ) -> BoxFuture<
            'e,
            Result<<Self::Database as sqlx::database::HasStatement<'q>>::Statement, sqlx::Error>,
        >
        where
            'c: 'e,
        {
            match self {
                PostgresExecutor::PoolExec(pool) => pool.prepare_with(sql, parameters),
                PostgresExecutor::TxExec(ref mut tx) => tx.prepare_with(sql, parameters),
            }
        }
    }

    #[derive(Clone)]
    pub struct PostgresDb {
        pool: PgPool,
    }

    impl PostgresDb {
        pub async fn connect(url: &str) -> Result<Self, DbError> {
            let connection_options = PgConnectOptions::from_str(&url)
                .map_err(|err| DbError::DatabaseUnavailable(err))?
                .application_name(env!("CARGO_PKG_NAME"))
                .statement_cache_capacity(250);

            let pool = sqlx::postgres::PgPoolOptions::new()
                .idle_timeout(std::time::Duration::from_secs(90))
                .max_lifetime(std::time::Duration::from_secs(1_800))
                .min_connections(1)
                .max_connections(16)
                .connect_with(connection_options)
                .await
                .map_err(|err| DbError::DatabaseUnavailable(err))?;

            Ok(Self { pool })
        }

        pub async fn migrate(&self) -> DbResult {
            MIGRATOR
                .run(&self.pool)
                .await
                .map_err(|err| DbError::MigrationFailed(err))
        }
    }

    impl PostgresDb {
        pub fn typed_ex(&self) -> PostgresExecutor {
            PostgresExecutor::PoolExec(self.pool.clone())
        }
    }

    #[async_trait]
    impl Db for PostgresDb {
        fn ex(&self) -> Executor {
            Executor::Postgres(self.typed_ex())
        }

        async fn begin(&self) -> DbResult<TxExecutor> {
            let tx = self.pool.begin().await.map_err(map_sqlx_error)?;
            Ok(TxExecutor(Executor::Postgres(PostgresExecutor::TxExec(tx))))
        }
    }

    pub fn map_sqlx_error(err: sqlx::Error) -> DbError {
        match err {
            sqlx::Error::ColumnDecode { .. } => DbError::CorruptData(err),
            sqlx::Error::Database(ref db_err) => {
                match db_err.downcast_ref::<PgDatabaseError>().code() {
                    "23503" /* foreign key violation */ => DbError::RecordNotFound,
                    "23505" /* unique violation */ => DbError::RecordExists,
                    "53300" /* too many connections */ => DbError::DatabaseUnavailable(err),
                    _ => DbError::InternalError(err),
                }
            },
            sqlx::Error::PoolTimedOut => DbError::DatabaseUnavailable(err),
            sqlx::Error::RowNotFound => DbError::RecordNotFound,
            err => DbError::InternalError(err),
        }
    }
}

#[cfg(feature="sqlite")]
pub mod sqlite {
    use std::str::FromStr;

    use axum::async_trait;
    use futures::future::BoxFuture;
    use sqlx::Transaction;
    use sqlx::migrate::Migrator;
    use sqlx::sqlite::{Sqlite, SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqliteSynchronous};

    use super::{Db, DbError, DbResult, Executor, TxExecutor};

    static MIGRATOR: Migrator = sqlx::migrate!("migrations/sqlite");

    #[derive(Clone)]
    pub struct SqliteDb {
        pool: SqlitePool,
    }

    impl SqliteDb {
        pub async fn connect(url: &str) -> DbResult<Self> {
            let connection_options = SqliteConnectOptions::from_str(url)
                .map_err(|err| DbError::DatabaseUnavailable(err))?
                .create_if_missing(true)
                .journal_mode(SqliteJournalMode::Wal)
                .statement_cache_capacity(250)
                .synchronous(SqliteSynchronous::Normal);

            let pool = sqlx::sqlite::SqlitePoolOptions::new()
                .idle_timeout(std::time::Duration::from_secs(90))
                .max_lifetime(std::time::Duration::from_secs(1_800))
                .min_connections(1)
                .max_connections(16)
                .connect_with(connection_options)
                .await
                .map_err(|err| DbError::DatabaseUnavailable(err))?;

            Ok(Self { pool })
        }

        pub async fn migrate(&self) -> DbResult {
            MIGRATOR
                .run(&self.pool)
                .await
                .map_err(|err| DbError::MigrationFailed(err))
        }

        pub fn typed_ex(&self) -> SqliteExecutor {
            SqliteExecutor::PoolExec(self.pool.clone())
        }
    }

    #[async_trait]
    impl Db for SqliteDb {
        fn ex(&self) -> Executor {
            Executor::Sqlite(SqliteExecutor::PoolExec(self.pool.clone()))
        }

        async fn begin(&self) -> DbResult<TxExecutor> {
            let tx = self.pool.begin().await.map_err(map_sqlx_error)?;
            Ok(TxExecutor(Executor::Sqlite(SqliteExecutor::TxExec(tx))))
        }
    }

    #[derive(Debug)]
    pub enum SqliteExecutor {
        PoolExec(SqlitePool),
        TxExec(Transaction<'static, Sqlite>),
    }

    impl SqliteExecutor {
        pub async fn commit(self) -> DbResult {
            match self {
                Self::PoolExec(_) => unreachable!("need to check this, but it shouldn't be called"),
                Self::TxExec(tx) => tx.commit().await.map_err(map_sqlx_error),
            }
        }
    }

    impl<'c> sqlx::Executor<'c> for &'c mut SqliteExecutor {
        type Database = Sqlite;

        fn describe<'e, 'q: 'e>(
            self,
            sql: &'q str,
        ) -> BoxFuture<'e, Result<sqlx::Describe<Self::Database>, sqlx::Error>>
        where
            'c: 'e,
        {
            match self {
                SqliteExecutor::PoolExec(pool) => pool.describe(sql),
                SqliteExecutor::TxExec(ref mut tx) => tx.describe(sql),
            }
        }

        fn fetch_many<'e, 'q: 'e, E: 'q>(
            self,
            query: E,
        ) -> futures::stream::BoxStream<
            'e,
            Result<
                sqlx::Either<
                    <Self::Database as sqlx::Database>::QueryResult,
                    <Self::Database as sqlx::Database>::Row,
                >,
                sqlx::Error,
            >,
        >
        where
            'c: 'e,
            E: sqlx::Execute<'q, Self::Database>,
        {
            match self {
                SqliteExecutor::PoolExec(pool) => pool.fetch_many(query),
                SqliteExecutor::TxExec(ref mut tx) => tx.fetch_many(query),
            }
        }

        fn fetch_optional<'e, 'q: 'e, E: 'q>(
            self,
            query: E,
        ) -> BoxFuture<'e, Result<Option<<Self::Database as sqlx::Database>::Row>, sqlx::Error>>
        where
            'c: 'e,
            E: sqlx::Execute<'q, Self::Database>,
        {
            match self {
                SqliteExecutor::PoolExec(pool) => pool.fetch_optional(query),
                SqliteExecutor::TxExec(ref mut tx) => tx.fetch_optional(query),
            }
        }

        fn prepare_with<'e, 'q: 'e>(
            self,
            sql: &'q str,
            parameters: &'e [<Self::Database as sqlx::Database>::TypeInfo],
        ) -> BoxFuture<
            'e,
            Result<<Self::Database as sqlx::database::HasStatement<'q>>::Statement, sqlx::Error>,
        >
        where
            'c: 'e,
        {
            match self {
                SqliteExecutor::PoolExec(pool) => pool.prepare_with(sql, parameters),
                SqliteExecutor::TxExec(ref mut tx) => tx.prepare_with(sql, parameters),
            }
        }
    }

    pub fn map_sqlx_error(err: sqlx::Error) -> DbError {
        match err {
            sqlx::Error::ColumnDecode { .. } => DbError::CorruptData(err),
            sqlx::Error::RowNotFound => DbError::RecordNotFound,
            err if err.to_string().contains("FOREIGN KEY constraint failed") => DbError::RecordNotFound,
            err if err.to_string().contains("UNIQUE constraint failed") => DbError::RecordExists,
            err => DbError::InternalError(err),
        }
    }
}

//#[axum::async_trait]
//pub trait DbPool: Sized {
//    type Database: sqlx::Database;
//
//    async fn begin(&self) -> DbSetupResult<DbExecutor<Self::Database>>;
//    async fn direct(&self) -> DbSetupResult<DbExecutor<Self::Database>>;
//
//    async fn is_migrated(&self) -> bool;
//    async fn run_migrations(&self) -> DbSetupResult<()>;
//}
//
//#[derive(Clone)]
//pub enum Db {
//    #[cfg(feature="postgres")]
//    Postgres(ProtectedDb<sqlx::Postgres>),
//
//    #[cfg(feature="sqlite")]
//    Sqlite(ProtectedDb<sqlx::Sqlite>),
//}
//
//impl Db {
//    pub async fn run_migrations(&self) -> DbSetupResult<()> {
//        match self {
//            #[cfg(feature="postgres")]
//            Db::Postgres(pdb) => pdb.run_migrations().await,
//
//            #[cfg(feature="sqlite")]
//            Db::Sqlite(pdb) => pdb.run_migrations().await,
//        }
//    }
//}
//
//#[derive(Debug)]
//pub enum DbExecutor<'a, T: sqlx::Database> {
//    Pool(sqlx::pool::Pool<T>),
//    Transaction(sqlx::Transaction<'a, T>),
//}
//
//impl<T: sqlx::Database> DbExecutor<'_, T> {
//    pub async fn commit(self) -> DbQueryResult<()> {
//        match self {
//            Self::Pool(_) => panic!("shouldn't commit on direct executors"),
//            Self::Transaction(tx) => tx.commit().await.map_err(map_query_error),
//        }
//    }
//}
//
////use sqlx::pool::PoolConnection;
////
////impl<'a, 'c, T: sqlx::Database> sqlx::Executor<'a> for &'a mut DbExecutor<'c, T>
////where
////    <T as sqlx::Database>::Connection: std::ops::DerefMut,
////    PoolConnection<T>: sqlx::Executor<'a, Database = T>
////{
////    type Database = T;
////
////    fn describe<'e, 'q: 'e>(
////        self,
////        sql: &'q str,
////    ) -> BoxFuture<'e, Result<sqlx::Describe<Self::Database>, sqlx::Error>>
////    where
////        'c: 'e,
////        'a: 'e,
////    {
////        match self {
////            DbExecutor::Pool(pool) => {
////                Box::pin(async move {
////                    let conn = pool.acquire().await?;
////                    conn.describe(sql).await
////                })
////            },
////            DbExecutor::Transaction(_) => panic!("can't describe in a transaction"),
////        }
////    }
////
////    fn fetch_many<'e, 'q, E>(
////        self,
////        query: E
////    ) -> BoxStream<'e, Result<sqlx::Either<<Self::Database as sqlx::Database>::QueryResult, <Self::Database as sqlx::Database>::Row>, sqlx::Error>>
////    where
////        'q: 'e,
////        'c: 'e,
////        E: 'q + sqlx::Execute<'q, Self::Database>
////    {
////        match self {
////            DbExecutor::Pool(pool) => pool.fetch_many(query),
////            DbExecutor::Transaction(tx) => tx.fetch_many(query),
////        }
////    }
////
////    fn fetch_optional<'e, 'q: 'e, E: 'q>(
////        self,
////        query: E,
////    ) -> BoxFuture<'e, Result<Option<<Self::Database as sqlx::Database>::Row>, sqlx::Error>>
////    where
////        'c: 'e,
////        E: sqlx::Execute<'q, Self::Database>
////    {
////        todo!()
////    }
////
////    fn prepare_with<'e, 'q: 'e>(
////        self,
////        sql: &'q str,
////        parameters: &'e [<Self::Database as sqlx::Database>::TypeInfo],
////    ) -> BoxFuture<'e, Result<<Self::Database as sqlx::database::HasStatement<'q>>::Statement, sqlx::Error>>
////    where
////        'c: 'e
////    {
////            todo!()
////    }
////}
//
//pub enum DbState {
//    Setup,
//    Migrating,
//    Ready,
//}
//
//pub struct ProtectedDb<T: sqlx::Database> {
//    pool: sqlx::pool::Pool<T>,
//    state: std::sync::Arc<tokio::sync::Mutex<DbState>>,
//}
//
//impl<T: sqlx::Database> Clone for ProtectedDb<T> {
//    fn clone(&self) -> Self {
//        Self {
//            pool: self.pool.clone(),
//            state: self.state.clone(),
//        }
//    }
//}
//
//impl<T: sqlx::Database> ProtectedDb<T> {
//    fn new(pool: sqlx::pool::Pool<T>) -> Self {
//        Self {
//            pool,
//            state: std::sync::Arc::new(tokio::sync::Mutex::new(DbState::Setup)),
//        }
//    }
//}
//
//#[cfg(feature="postgres")]
//#[axum::async_trait]
//impl DbPool for ProtectedDb<sqlx::Postgres> {
//    type Database = sqlx::Postgres;
//
//    async fn begin(&self) -> DbSetupResult<DbExecutor<Self::Database>> {
//        match &*self.state.lock().await {
//            DbState::Setup => return Err(DbSetupError::MigrationRequired),
//            DbState::Migrating => return Err(DbSetupError::MigrationInProgress),
//            DbState::Ready => (),
//        }
//
//        let tx = self.pool.begin().await.map_err(|err| DbSetupError::DatabaseUnavailable(err))?;
//        Ok(DbExecutor::Transaction(tx))
//    }
//
//    async fn direct(&self) -> DbSetupResult<DbExecutor<Self::Database>> {
//        match &*self.state.lock().await {
//            DbState::Setup => return Err(DbSetupError::MigrationRequired),
//            DbState::Migrating => return Err(DbSetupError::MigrationInProgress),
//            DbState::Ready => (),
//        }
//
//        Ok(DbExecutor::Pool(self.pool.clone()))
//    }
//
//    async fn is_migrated(&self) -> bool {
//        let state = self.state.lock().await;
//        matches!(&*state, DbState::Ready)
//    }
//
//    async fn run_migrations(&self) -> DbSetupResult<()> {
//        let mut state = self.state.lock().await;
//
//        match &*state {
//            DbState::Setup => (),
//            DbState::Migrating => return Err(DbSetupError::MigrationInProgress),
//            DbState::Ready => return Ok(()),
//        }
//
//        *state = DbState::Migrating;
//        drop(state);
//
//        postgres::run_migrations(&self.pool).await?;
//
//        let mut state = self.state.lock().await;
//        *state = DbState::Ready;
//
//        Ok(())
//    }
//}
//
//#[cfg(feature="sqlite")]
//#[axum::async_trait]
//impl DbPool for ProtectedDb<sqlx::Sqlite> {
//    type Database = sqlx::Sqlite;
//
//    async fn begin(&self) -> DbSetupResult<DbExecutor<Self::Database>> {
//        match &*self.state.lock().await {
//            DbState::Setup => return Err(DbSetupError::MigrationRequired),
//            DbState::Migrating => return Err(DbSetupError::MigrationInProgress),
//            DbState::Ready => (),
//        }
//
//        let tx = self.pool.begin().await.map_err(|err| DbSetupError::DatabaseUnavailable(err))?;
//        Ok(DbExecutor::Transaction(tx))
//    }
//
//    async fn direct(&self) -> DbSetupResult<DbExecutor<Self::Database>> {
//        match &*self.state.lock().await {
//            DbState::Setup => return Err(DbSetupError::MigrationRequired),
//            DbState::Migrating => return Err(DbSetupError::MigrationInProgress),
//            DbState::Ready => (),
//        }
//
//        Ok(DbExecutor::Pool(self.pool.clone()))
//    }
//
//    async fn is_migrated(&self) -> bool {
//        let state = self.state.lock().await;
//        matches!(&*state, DbState::Ready)
//    }
//
//    async fn run_migrations(&self) -> DbSetupResult<()> {
//        let mut state = self.state.lock().await;
//
//        match &*state {
//            DbState::Setup => (),
//            // todo: I could have a failed state, and include a counter on the migration attempt to
//            // allow some background task to periodically retry
//            DbState::Migrating => return Err(DbSetupError::MigrationInProgress),
//            DbState::Ready => return Ok(()),
//        }
//
//        *state = DbState::Migrating;
//        drop(state);
//
//        sqlite::run_migrations(&self.pool).await?;
//
//        let mut state = self.state.lock().await;
//        *state = DbState::Ready;
//
//        Ok(())
//    }
//}
