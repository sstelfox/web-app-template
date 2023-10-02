use std::collections::HashSet;
use std::sync::OnceLock;

use axum::{async_trait, Json, RequestPartsExt};
use axum::extract::{FromRef, FromRequestParts, TypedHeader};
use axum::extract::rejection::TypedHeaderRejection;
use axum::headers::{Authorization, Cookie};
use axum::headers::authorization::Bearer;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use http::request::Parts;
use jwt_simple::prelude::*;
use regex::Regex;
use uuid::Uuid;

use crate::app::SessionVerificationKey;

const MAXIMUM_SESSION_AGE: u64 = 24 * 7 * 60 * 60; // 1 week

static SESSION_COOKIE_CONTENT_PATTERN: &str = r"^[0-9a-f]{64}$";

static SESSION_COOKIE_CONTENT_VALIDATOR: OnceLock<Regex> = OnceLock::new();

pub struct SessionIdentity {
    user_id: Uuid,
}

impl SessionIdentity {
    pub fn user_id(&self) -> &Uuid {
        &self.user_id
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for SessionIdentity
where
    SessionVerificationKey: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = SessionIdentityError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let key_validator = SESSION_COOKIE_CONTENT_VALIDATOR.get_or_init(|| Regex::new(SESSION_COOKIE_CONTENT_PATTERN).unwrap());

        let TypedHeader(cookie) = parts
            .extract::<TypedHeader<Cookie>>()
            .await
            .map_err(|err| Self::Rejection::MissingHeader(err))?;

        //let raw_token = bearer.token();

        //let unvalidated_header = Token::decode_metadata(&raw_token).map_err(|err| Self::Rejection::CorruptHeader(err))?;
        //let key_id = match unvalidated_header.key_id() {
        //    Some(kid) if key_validator.is_match(kid) => kid.to_string(),
        //    Some(_) => return Err(Self::Rejection::InvalidKeyId),
        //    None => return Err(Self::Rejection::MissingKeyId),
        //};

        //let jwt_key = JwtKey::from_request_parts(parts, state)
        //    .await
        //    .map_err(|_| Self::Rejection::key_unavailable())?;

        //let verification_options = VerificationOptions {
        //    accept_future: false,
        //    // todo: tokens should be intended for us, make this a configurable service name we can
        //    // re-use and reference
        //    allowed_audiences: Some(HashSet::from_strings(&["web-app-template"])),
        //    max_validity: Some(Duration::from_secs(MAXIMUM_TOKEN_AGE)),
        //    time_tolerance: Some(Duration::from_secs(15)),
        //    ..Default::default()
        //};

        //let claims = jwt_key
        //    .as_ref()
        //    .public_key()
        //    .verify_token::<NoCustomClaims>(&raw_token, Some(verification_options))
        //    .map_err(Self::Rejection::validation_failed)?;

        //if claims.nonce.is_none() {
        //    return Err(Self::Rejection::NonceMissing);
        //}

        //// todo: validate subject is present, do I need any extra validation?
        //let user_id = match &claims.subject {
        //    Some(sub) => Uuid::parse_str(sub).map_err(|_| Self::Rejection::SubjectInvalid)?,
        //    None => return Err(Self::Rejection::SubjectMissing),
        //};

        //Ok(SessionIdentity { user_id })
        todo!()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SessionIdentityError {
    //#[error("provided JWT had an invalid or corrupt header")]
    //CorruptHeader(jwt_simple::Error),

    //#[error("key ID included in JWT header did not match our expected format")]
    //InvalidKeyId,

    #[error("authenticated route was missing authorization header")]
    MissingHeader(TypedHeaderRejection),

    //#[error("no key ID was included in the JWT header")]
    //MissingKeyId,

    //#[error("no nonce was included in the token")]
    //NonceMissing,

    //#[error("provided subject was not a valid UUID")]
    //SubjectInvalid,

    //#[error("no subject was included in the token")]
    //SubjectMissing,

    //#[error("validation of the provided JWT failed")]
    //ValidationFailed(jwt_simple::Error),
}

impl IntoResponse for SessionIdentityError {
    fn into_response(self) -> Response {
        use SessionIdentityError as SIE;

        match self {
            SIE::MissingHeader(_) => {
                tracing::error!("no cookie header in request to check for session identity");
                let err_msg = serde_json::json!({ "msg": "no authentication material found in request" });
                (StatusCode::INTERNAL_SERVER_ERROR, Json(err_msg)).into_response()
            },
        }
    }
}
