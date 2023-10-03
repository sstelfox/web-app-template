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
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
use http::request::Parts;
use jwt_simple::prelude::*;
use regex::Regex;
use uuid::Uuid;

use crate::app::SessionVerificationKey;

static LOGIN_PATH: &str = "/auth/login";

const MAX_COOKIE_LENGTH: usize = 77;

static SESSION_COOKIE_NAME: &str = "_session_id";

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
        let cookie_jar: CookieJar = CookieJar::from_headers(&parts.headers);

        let session_cookie = match cookie_jar.get(SESSION_COOKIE_NAME) {
            Some(st) => st,
            None => {
                let OriginalUri(uri) = OriginalUri::from_request_parts(parts, state).await.expect("infallible conversion");
                return Err(SessionIdentityError::NoSession(uri.to_string()));
            }
        };

        // todo: some sanity checks on the cookie (path, security, is web only)

        let raw_cookie_val = session_cookie.value();
        if raw_cookie_val.len() >= MAX_COOKIE_LENGTH {
            return Err(SessionIdentityError::CookieTooLarge);
        }

        let mut cookie_pieces = raw_cookie_val.split('*');

        let session_id_b64 = cookie_pieces.next().ok_or(SessionIdentityError::EncodingError)?;
        let session_id_bytes = B64.decode(session_id_b64).map_err(|_| SessionIdentityError::EncodingError)?;
        if session_id_bytes.len() != 8 {
            return Err(SessionIdentityError::EncodingError);
        }

        let digest_b64 = cookie_pieces.next().ok_or(SessionIdentityError::EncodingError)?;
        let digest_bytes = B64.decode(digest_b64).map_err(|_| SessionIdentityError::EncodingError)?;

        let verification_key = SessionVerificationKey::from_ref(state).public_key();

        //Ok(SessionIdentity { user_id })
        todo!()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SessionIdentityError {
    #[error("received cookie that was larger than we expect or accept")]
    CookieTooLarge,

    #[error("cookie was not encoded into the correct format")]
    EncodingError,

    #[error("user didn't have an existing session")]
    NoSession(String),
}

impl IntoResponse for SessionIdentityError {
    fn into_response(self) -> Response {
        use SessionIdentityError as SIE;

        // todo: may want to consolidate actions here with some helper methods, may want to include
        // the original uri in the get request to the appropriate auth server and record it in the
        // session db...

        // todo: handle redirecting back to the original uri

        match self {
            SIE::NoSession(_orig_uri) => {
                tracing::debug!("request had no session when trying to access protected path");
                Redirect::to(LOGIN_PATH).into_response()
            },
            err => {
                tracing::warn!("session validation error: {err}");
                // Clear all cookies and send them to the login path
                (CookieJar::new(), Redirect::to(LOGIN_PATH)).into_response()
            }
        }
    }
}
