pub mod custom_types;
pub mod models;
pub mod sqlite;

use std::convert::Infallible;
use std::ops::Deref;

use axum::async_trait;
use axum::extract::{FromRef, FromRequestParts};
use http::request::Parts;
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct Database(SqlitePool);

impl Database {
    pub async fn connect(db_url: &url::Url) -> Result<Self, DatabaseSetupError> {
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
            sqlite::migrate_sqlite(&db).await?;
            return Ok(Database::new(db));
        }

        Err(DatabaseSetupError::UnknownDbType(
            db_url.scheme().to_string(),
        ))
    }

    pub fn new(pool: SqlitePool) -> Self {
        Self(pool)
    }
}

impl Deref for Database {
    type Target = SqlitePool;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for Database
where
    Database: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(_parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        Ok(Database::from_ref(state))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DatabaseSetupError {
    #[error("error occurred while attempting database migration: {0}")]
    MigrationFailed(sqlx::migrate::MigrateError),

    #[error("unable to perform initial connection and check of the database: {0}")]
    Unavailable(sqlx::Error),

    #[error("requested database type was not recognized: {0}")]
    UnknownDbType(String),
}
