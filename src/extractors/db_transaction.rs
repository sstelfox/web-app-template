use std::ops::Deref;

use axum::{async_trait, Json};
use axum::extract::{FromRef, FromRequestParts};
use axum::response::{IntoResponse, Response};
use http::StatusCode;

use crate::database::{Database, DbError, TxExecutor};

pub struct DbTransaction(TxExecutor);

impl Deref for DbTransaction {
    type Target = TxExecutor;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for DbTransaction
where
    Database: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = DbTransactionError;

    async fn from_request_parts(
        _parts: &mut http::request::Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let database = Database::from_ref(state);
        let transaction = database.begin().await.map_err(|err| DbTransactionError::BeginFailed(err))?;
        Ok(Self(transaction))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DbTransactionError {
    #[error("unable to begin transaction")]
    BeginFailed(DbError),
}

impl IntoResponse for DbTransactionError {
    fn into_response(self) -> Response {
        use DbTransactionError::*;

        match self {
            BeginFailed(err) => {
                tracing::error!(err = ?err, "unable to begin new transaction");
                let err_msg = serde_json::json!({ "status": "database unavailable" });
                (StatusCode::INTERNAL_SERVER_ERROR, Json(err_msg)).into_response()
            }
        }
    }
}
