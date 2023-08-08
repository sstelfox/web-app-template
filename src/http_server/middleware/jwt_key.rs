use std::ops::Deref;
use std::sync::Arc;

use axum::async_trait;
use axum::extract::{FromRef, FromRequestParts};
use jwt_simple::algorithms::ES384KeyPair;

pub struct JwtKey(Arc<ES384KeyPair>);

impl Deref for JwtKey {
    type Target = Arc<ES384KeyPair>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for JwtKey
where
    Arc<ES384KeyPair>: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = ();

    async fn from_request_parts(
        _parts: &mut http::request::Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        Ok(Self(Arc::<ES384KeyPair>::from_ref(state)))
    }
}
