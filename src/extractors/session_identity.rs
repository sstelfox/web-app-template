use std::collections::HashSet;
use std::sync::OnceLock;

use axum::extract::rejection::TypedHeaderRejection;
use axum::extract::{FromRef, FromRequestParts, OriginalUri, TypedHeader};
use axum::headers::authorization::Bearer;
use axum::headers::Authorization;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::{async_trait, Json, RequestPartsExt};
use axum_extra::extract::cookie::{Cookie, CookieJar};
use http::request::Parts;
use jwt_simple::prelude::*;
use regex::Regex;
use uuid::Uuid;

use crate::app::SessionVerificationKey;

static LOGIN_PATH: &str = "/auth/login";

const MAXIMUM_SESSION_AGE: u64 = 24 * 7 * 60 * 60; // 1 week

static SESSION_COOKIE_NAME: &str = "_session_id";

static SESSION_COOKIE_KID_PATTERN: &str = "^[0-9a-f]{64}$";

static SESSION_COOKIE_KID_VALIDATOR: OnceLock<Regex> = OnceLock::new();

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
        let kid_validator = SESSION_COOKIE_KID_VALIDATOR
            .get_or_init(|| Regex::new(SESSION_COOKIE_KID_PATTERN).unwrap());

        let cookie_jar: CookieJar = CookieJar::from_headers(&parts.headers);

        let session_cookie = match cookie_jar.get(SESSION_COOKIE_NAME) {
            Some(st) => st,
            None => {
                let OriginalUri(uri) = OriginalUri::from_request_parts(parts, state).await.expect("infallible conversion");
                return Err(SessionIdentityError::NoSession(uri.to_string()));
            }
        };

        // todo: some sanity checks on the cookie (path, security, is web only)

        let unvalidated_header = Token::decode_metadata(session_cookie.value()).map_err(|err| Self::Rejection::CorruptHeader(err))?;
        let key_id = match unvalidated_header.key_id() {
            Some(kid) if kid_validator.is_match(kid) => kid.to_string(),
            Some(_) => return Err(Self::Rejection::InvalidKeyId),
            None => return Err(Self::Rejection::MissingKeyId),
        };

        let verification_key = SessionVerificationKey::from_ref(state);

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
    #[error("session JWT had an invalid or corrupt header")]
    CorruptHeader(jwt_simple::Error),

    #[error("key ID included in sesion JWT header did not match our expected format")]
    InvalidKeyId,

    #[error("no key ID was included in the session JWT header")]
    MissingKeyId,

    #[error("user didn't have an existing session")]
    NoSession(String),

    //#[error("authenticated route was missing authorization header")]
    //MissingHeader(TypedHeaderRejection),

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

        // todo: may want to consolidate actions here with some helper methods, may want to include
        // the original uri in the get request to the appropriate auth server and record it in the
        // session db...

        match self {
            SIE::CorruptHeader(err) => {
                tracing::error!("session header appears to be corrupted (err); forcing login");
                (CookieJar::new(), Redirect::to(LOGIN_PATH)).into_response()
            },
            SIE::InvalidKeyId => {
                tracing::error!("provided session key ID did not match expected session key");
                (CookieJar::new(), Redirect::to(LOGIN_PATH)).into_response()
            },
            SIE::MissingKeyId => {
                tracing::error!("no key ID in session JWT");
                (CookieJar::new(), Redirect::to(LOGIN_PATH)).into_response()
            },
            SIE::NoSession(_orig_uri) => {
                // todo: handle redirecting back to the original uri
                tracing::debug!("request had no session when trying to access protected path");
                Redirect::to(LOGIN_PATH).into_response()
            },
        }
    }
}
