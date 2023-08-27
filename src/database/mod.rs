use std::sync::Arc;

use axum::async_trait;

use crate::app::Config;

//#[cfg(all(feature = "postgres", feature = "sqlite"))]
//compile_error!("Database selection features `postgres` and `sqlite` are mutually exclusive, you cannot enable both!");

#[cfg(not(any(feature = "postgres", feature = "sqlite")))]
compile_error!("You must enable at least one database features: `postgres` or `sqlite`");

pub async fn config_database(config: &Config) -> DbResult<Arc<dyn Db + Send + Sync>> {
    // todo: I should figure out a way to delay the actual connection and running of migrations,
    // and reflect the service being unavailable in the readiness check until they're complete. If
    // our connection fails we should try a couple of times with a backoff before failing the
    // entire service...
    //
    // maybe a tokio task with a channel or shared state directly that can be consumed by the
    // healthcheck and database extractor... Maybe this state belongs on the database executor
    // itself...

    let db = match config.db_url() {
        //#[cfg(feature="postgres")]
        //db_url if db_url.starts_with("postgres://") => {
        //    let pool = postgres::configure_pool(db_url.as_str()).await?;
        //    Db::Postgres(ProtectedDb::new(pool))
        //}

        #[cfg(feature="sqlite")]
        db_url if db_url.starts_with("sqlite://") => {
            let pool = sqlite::SqliteDb::connect(db_url).await?;
            Arc::new(pool)
        }

        _ => panic!("unknown database type, unable to setup database"),
    };

    //db.run_migrations().await?;

    Ok(db)
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

    #[error("unable to create record as it would violate a uniqueness constraint")]
    RecordExists,

    #[error("unable to locate record or associated foreign key")]
    RecordNotFound,
}

pub type DbResult<T = ()> = Result<T, DbError>;

pub enum Executor {
    //Postgres(postgres::PostgresExecutor),
    Sqlite(sqlite::SqliteExecutor),
}

pub struct TxExecutor(Executor);

impl TxExecutor {
    pub fn ex(&mut self) -> &mut Executor {
        &mut self.0
    }

    pub async fn commit(self) -> DbResult {
        match self.0 {
            //Executor::Postgres(e) => e.commit().await,
            Executor::Sqlite(e) => e.commit().await,
        }
    }
}

//pub mod postgres {
//    use sqlx::Transaction;
//
//    use super::{DbError, DbResult};
//
//}

pub mod sqlite {
    use std::str::FromStr;

    use axum::async_trait;
    use futures::future::BoxFuture;
    use sqlx::Transaction;
    use sqlx::sqlite::{Sqlite, SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqliteSynchronous};

    use super::{Db, DbError, DbResult, Executor, TxExecutor};

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

//#[cfg(feature="postgres")]
//mod postgres;
//
//#[cfg(feature="sqlite")]
//mod sqlite;
//
//pub type DbQueryResult<T> = Result<T, DbQueryError>;
//
//pub type DbSetupResult<T> = Result<T, DbSetupError>;
//
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
//fn map_query_error(err: sqlx::Error) -> DbQueryError {
//    match err {
//        sqlx::Error::ColumnDecode { .. } => DbQueryError::CorruptData(err),
//        //sqls::Error::Database(db_err) => {
//        //    match db_err.downcast_ref::<sqlx::postgres::PgDatabaseError>().code() {
//        //        "23503" /* foreign_key_violation */ => DbQueryError::NotFound,
//        //        "23505" /* unique violation */ => DbQueryError::RecordAlreadyExists,
//        //        // would be covered by fallback
//        //        "53300" /* to many connections */ => DbQueryError::DatabaseUnavailable(err),
//        //        _ => DbQueryError::DatabaseUnavailable(err),
//        //    }
//        //},
//        sqlx::Error::RowNotFound => DbQueryError::RecordNotFound,
//        _ => DbQueryError::DatabaseUnavailable(err),
//    }
//}
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

//#[derive(Debug, thiserror::Error)]
//pub enum DbQueryError {
//    #[error("data loaded in from the database failed our validation and is presumed corrupt")]
//    CorruptData(sqlx::Error),
//
//    #[error("unable to get a connection to the database")]
//    DatabaseUnavailable(sqlx::Error),
//
//    #[error("failed to create record as another one matching the uniqueness restrictions was found")]
//    RecordAlreadyExists,
//
//    #[error("unable to locate the requested record")]
//    RecordNotFound,
//}
