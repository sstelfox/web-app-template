#![allow(dead_code)]

use std::collections::HashSet;
use std::sync::OnceLock;

use axum::extract::{FromRef, FromRequestParts};
use axum::response::{IntoResponse, Response};
use axum::{async_trait, Json, RequestPartsExt};
use axum_extra::typed_header::TypedHeaderRejection;
use axum_extra::TypedHeader;
use headers::authorization::Bearer;
use headers::Authorization;
use http::request::Parts;
use http::StatusCode;
use jwt_simple::prelude::*;
use regex::Regex;
use uuid::Uuid;

use crate::database::custom_types::Fingerprint;
use crate::database::models::ApiKey;
use crate::database::Database;

/// Defines the maximum length of time we consider any individual token valid in seconds. If the
/// expiration is still in the future, but it was issued more than this many seconds in the past
/// we'll reject the token even if its otherwise valid.
const MAXIMUM_TOKEN_AGE: u64 = 900;

static KEY_ID_PATTERN: &str = r"^[0-9a-f]{64}$";

static KEY_ID_VALIDATOR: OnceLock<Regex> = OnceLock::new();

pub struct ApiKeyIdentity {
    user_id: Uuid,
    key_id: String,
}

impl ApiKeyIdentity {
    pub fn key_id(&self) -> &str {
        self.key_id.as_str()
    }

    pub fn user_id(&self) -> &Uuid {
        &self.user_id
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for ApiKeyIdentity
where
    Database: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = ApiKeyIdentityError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let key_validator = KEY_ID_VALIDATOR.get_or_init(|| Regex::new(KEY_ID_PATTERN).unwrap());

        let TypedHeader(Authorization(bearer)) = parts
            .extract::<TypedHeader<Authorization<Bearer>>>()
            .await
            .map_err(ApiKeyIdentityError::MissingHeader)?;

        let raw_token = bearer.token();

        let unvalidated_header =
            Token::decode_metadata(raw_token).map_err(ApiKeyIdentityError::CorruptHeader)?;

        let key_id = match unvalidated_header.key_id() {
            Some(kid) if key_validator.is_match(kid) => kid.to_string(),
            Some(_) => return Err(ApiKeyIdentityError::InvalidKeyId),
            None => return Err(ApiKeyIdentityError::MissingKeyId),
        };

        let database = Database::from_ref(state);
        let mut conn = database
            .acquire()
            .await
            .map_err(ApiKeyIdentityError::DatabaseUnavailable)?;

        let fingerprint = Fingerprint::from_hex_str(&key_id).expect("valid fingerprint");
        let _api_key = ApiKey::from_fingerprint(&fingerprint);

        // todo create a generic "SessionKeyProvider" that takes a key ID and returns an
        //   appropriate verification key, should use that instead of a JwtKey directly
        //   I can implement a static provider that matches the token key against our regular
        //   one.
        //
        //#[axum::async_trait]
        //trait SessionKeyProvider {
        //    type Error: std::error::Error + Send + Sync;
        //
        //    async fn lookup(key_id: &str) -> Result<SessionKey, Self::Error>;
        //}

        let _verification_options = VerificationOptions {
            accept_future: false,
            // todo: tokens should be intended for us, make this a configurable service name we can
            // re-use and reference
            allowed_audiences: Some(HashSet::from_strings(&[env!("CARGO_PKG_NAME")])),
            max_validity: Some(Duration::from_secs(MAXIMUM_TOKEN_AGE)),
            time_tolerance: Some(Duration::from_secs(15)),
            ..Default::default()
        };

        //let claims = jwt_key
        //    .as_ref()
        //    .public_key()
        //    .verify_token::<NoCustomClaims>(&raw_token, Some(verification_options))
        //    .map_err(Self::Rejection::validation_failed)?;

        //if claims.nonce.is_none() {
        //    return Err(Self::Rejection::NonceMissing);
        //}

        // TODO: When the JWT is validated we should record the issued_at timestamp and record it
        // associated to the specific API key. Future requests should compare against the issued
        // time to prevent replay attacks from old tokens. We do keep the token age short to limit
        // the possibility of this happening and should also check based on IP. Might want to treat
        // these as sessions of a sort even to capture the same kind of metrics and streamline
        // authorization checks into a single session type.

        //// todo: validate subject is present, do I need any extra validation?
        //tracing::info!("{claims:?}");
        //let user_id = match &claims.subject {
        //    Some(sub) => Uuid::parse_str(sub).map_err(|_| Self::Rejection::SubjectInvalid)?,
        //    None => return Err(Self::Rejection::SubjectMissing),
        //};

        //Ok(ApiKeyIdentity { user_id, key_id })
        todo!()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ApiKeyIdentityError {
    #[error("provided JWT had an invalid or corrupt header")]
    CorruptHeader(jwt_simple::Error),

    #[error("database connection error: {0}")]
    DatabaseUnavailable(sqlx::Error),

    #[error("key ID included in JWT header did not match our expected format")]
    InvalidKeyId,

    #[error("unable to find JWT verification key in server state")]
    KeyUnavailable,

    #[error("authenticated route was missing authorization header")]
    MissingHeader(TypedHeaderRejection),

    #[error("no key ID was included in the JWT header")]
    MissingKeyId,

    #[error("no nonce was included in the token")]
    NonceMissing,

    #[error("provided subject was not a valid UUID")]
    SubjectInvalid,

    #[error("no subject was included in the token")]
    SubjectMissing,

    #[error("validation of the provided JWT failed")]
    ValidationFailed(jwt_simple::Error),
}

impl IntoResponse for ApiKeyIdentityError {
    fn into_response(self) -> Response {
        use ApiKeyIdentityError::*;

        match self {
            KeyUnavailable => {
                let err_msg =
                    serde_json::json!({ "status": "authentication services unavailable" });
                (StatusCode::INTERNAL_SERVER_ERROR, Json(err_msg)).into_response()
            }
            _ => {
                let err_msg = serde_json::json!({ "status": "invalid bearer token" });
                (StatusCode::BAD_REQUEST, Json(err_msg)).into_response()
            }
        }
    }
}
