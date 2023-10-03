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
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
use base64::Engine;
use ecdsa::signature::DigestVerifier;
use http::request::Parts;
use jwt_simple::prelude::*;
use regex::Regex;
use uuid::Uuid;

use crate::app::SessionVerificationKey;

static LOGIN_PATH: &str = "/auth/login";

static SESSION_COOKIE_NAME: &str = "_session_id";

pub struct SessionIdentity {
    session_id: Uuid,
    user_id: Uuid,
}

impl SessionIdentity {
    pub fn session_id(&self) -> Uuid {
        self.session_id
    }

    pub fn user_id(&self) -> Uuid {
        self.user_id
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
                let OriginalUri(uri) = OriginalUri::from_request_parts(parts, state)
                    .await
                    .expect("infallible conversion");
                return Err(SessionIdentityError::NoSession(uri.to_string()));
            }
        };

        // todo: some sanity checks on the cookie (path, security, is web only)

        let raw_cookie_val = session_cookie.value();

        // todo: these are going to be fixed lengths, validate the length and switch to split_at
        let mut cookie_pieces = raw_cookie_val.split('*');

        let session_id_b64 = cookie_pieces
            .next()
            .ok_or(SessionIdentityError::EncodingError)?;
        let authentication_tag_b64 = cookie_pieces
            .next()
            .ok_or(SessionIdentityError::EncodingError)?;

        if cookie_pieces.next().is_some() {
            return Err(SessionIdentityError::EncodingError);
        }

        let authentication_tag_bytes = B64
            .decode(authentication_tag_b64)
            .map_err(|_| SessionIdentityError::EncodingError)?;

        let ecdsa_signature = ecdsa::Signature::try_from(authentication_tag_bytes.as_slice())
            .map_err(SessionIdentityError::InvalidSignatureBytes)?;
        let mut digest = hmac_sha512::sha384::Hash::new();
        digest.update(&session_id_b64);

        let verification_key = SessionVerificationKey::from_ref(state);
        verification_key
            .public_key()
            .as_ref()
            .verify_digest(digest, &ecdsa_signature)
            .map_err(SessionIdentityError::BadSignature)?;

        // We now know these are good bytes, decode them, turn them into a valid session ID and
        // check the DB for them...

        let session_id_bytes = B64
            .decode(session_id_b64)
            .map_err(|_| SessionIdentityError::EncodingError)?;

        let session_id_bytes: [u8; 16] = session_id_bytes.try_into().expect("signed session ID to be valid byte slice");
        let session_id = Uuid::from_bytes_le(session_id_bytes);

        // todo: lookup session id in the db

        //Ok(SessionIdentity { user_id })
        todo!()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SessionIdentityError {
    #[error("signature did not match digest, tampering likely: {0}")]
    BadSignature(ecdsa::Error),

    #[error("received cookie that was larger than we expect or accept")]
    CookieTooLarge,

    #[error("cookie was not encoded into the correct format")]
    EncodingError,

    #[error("authenicated signature was in a valid format: {0}")]
    InvalidSignatureBytes(ecdsa::Error),

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
            }
            err => {
                tracing::warn!("session validation error: {err}");
                // Clear all cookies and send them to the login path
                (CookieJar::new(), Redirect::to(LOGIN_PATH)).into_response()
            }
        }
    }
}
