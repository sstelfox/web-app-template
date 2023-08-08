use std::sync::OnceLock;

use axum::{async_trait, Json, RequestPartsExt};
use axum::extract::{FromRef, FromRequestParts, TypedHeader};
use axum::extract::rejection::TypedHeaderRejection;
use axum::headers::Authorization;
use axum::headers::authorization::Bearer;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use http::request::Parts;
use jwt_simple::prelude::*;
use regex::Regex;
use uuid::Uuid;

use crate::http_server::middleware::JwtKey;

pub const EXPIRATION_WINDOW_SECS: usize = 900;

static KEY_ID_PATTERN: &str = r"^[0-9a-f]{64}$";

static KEY_ID_VALIDATOR: OnceLock<Regex> = OnceLock::new();

pub struct ApiKeyIdentity {
    user_id: Uuid,
}

#[async_trait]
impl<S> FromRequestParts<S> for ApiKeyIdentity
where
    JwtKey: FromRequestParts<S>,
    S: Send + Sync,
{
    type Rejection = ApiKeyIdentityError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let key_validator = KEY_ID_VALIDATOR.get_or_init(|| Regex::new(KEY_ID_PATTERN).unwrap());

        let TypedHeader(Authorization(bearer)) = parts
            .extract::<TypedHeader<Authorization<Bearer>>>()
            .await
            .map_err(ApiKeyIdentityError::missing_header)?;

        let raw_token = bearer.token();

        let unvalidated_header = Token::decode_metadata(&raw_token).map_err(ApiKeyIdentityError::corrupt_header)?;
        let _key_id = match unvalidated_header.key_id() {
            Some(kid) if key_validator.is_match(kid) => kid,
            Some(_) => return Err(ApiKeyIdentityError::InvalidKeyId),
            None => return Err(ApiKeyIdentityError::MissingKeyId),
        };

        // todo extract key ID, do header verification, whatever else we need

        // todo create a generic "SessionKeyProvider" that takes a key ID and returns an
        //   appropriate verification key, should use that instead of a JwtKey directly
        //   I can implement a static provider that matches the token key against our regular
        //   one.

        let jwt_key = JwtKey::from_request_parts(parts, state)
            .await
            .map_err(|_| ApiKeyIdentityError::key_unavailable())?;

        Ok(ApiKeyIdentity {
            user_id: Uuid::new_v4(),
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ApiKeyIdentityError {
    #[error("provided JWT had an invalid or corrupt header")]
    CorruptHeader(jwt_simple::Error),

    #[error("key ID included in JWT header did not match our expected format")]
    InvalidKeyId,

    #[error("unable to find JWT verification key in server state")]
    KeyUnavailable,

    #[error("authenticated route was missing authorization header")]
    MissingHeader(TypedHeaderRejection),

    #[error("no key ID was included in the JWT header")]
    MissingKeyId,
}

impl ApiKeyIdentityError {
    fn corrupt_header(err: jwt_simple::Error) -> Self {
        Self::CorruptHeader(err)
    }

    fn key_unavailable() -> Self {
        Self::KeyUnavailable
    }

    fn missing_header(err: TypedHeaderRejection) -> Self {
        Self::MissingHeader(err)
    }
}

impl IntoResponse for ApiKeyIdentityError {
    fn into_response(self) -> Response {
        use ApiKeyIdentityError::*;

        match self {
            CorruptHeader(_) | InvalidKeyId | MissingHeader(_) | MissingKeyId => {
                let err_msg = serde_json::json!({ "status": "invalid bearer token" });
                (StatusCode::BAD_REQUEST, Json(err_msg)).into_response()
            },
            KeyUnavailable => {
                let err_msg = serde_json::json!({ "status": "authentication services unavailable" });
                (StatusCode::INTERNAL_SERVER_ERROR, Json(err_msg)).into_response()
            },
        }
    }
}
