use std::convert::Infallible;

use axum::async_trait;
use axum::extract::{FromRef, FromRequestParts};
use http::request::Parts;

use crate::app::State;
use crate::database::Database;

#[async_trait]
impl FromRequestParts<State> for Database {
    type Rejection = Infallible;

    async fn from_request_parts(_parts: &mut Parts, state: &State) -> Result<Self, Self::Rejection> {
        Ok(Database::from_ref(state))
    }
}
