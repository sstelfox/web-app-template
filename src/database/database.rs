use std::convert::Infallible;
use std::ops::Deref;

use axum::async_trait;
use axum::extract::{FromRef, FromRequestParts};
use http::request::Parts;
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct Database(SqlitePool);

impl Database {
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
