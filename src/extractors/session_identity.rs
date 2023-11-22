use axum::async_trait;
use axum::extract::{FromRef, FromRequestParts, OriginalUri};
use axum::response::{IntoResponse, Redirect, Response};
use axum_extra::extract::cookie::CookieJar;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
use base64::Engine;
use ecdsa::signature::DigestVerifier;
use http::request::Parts;
use jwt_simple::prelude::*;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::app::ServiceVerificationKey;
use crate::auth::{LOGIN_PATH, SESSION_COOKIE_NAME};
use crate::database::custom_types::{OAuthProviderAccountId, SessionId, UserId};
use crate::database::models::Session;
use crate::database::Database;
use crate::utils::remove_cookie;

pub struct SessionIdentity {
    id: SessionId,
    provider_account_id: OAuthProviderAccountId,
    user_id: UserId,

    created_at: OffsetDateTime,
    expires_at: OffsetDateTime,
}

impl SessionIdentity {
    pub fn created_at(&self) -> &OffsetDateTime {
        &self.created_at
    }

    pub fn expires_at(&self) -> &OffsetDateTime {
        &self.expires_at
    }

    pub fn id(&self) -> SessionId {
        self.id
    }

    pub fn provider_account_id(&self) -> OAuthProviderAccountId {
        self.provider_account_id
    }

    pub fn user_id(&self) -> UserId {
        self.user_id
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for SessionIdentity
where
    Database: FromRef<S>,
    ServiceVerificationKey: FromRef<S>,
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
        if raw_cookie_val.len() != 150 {
            // 22 bytes digest, 128 bytes hmac
            // invalid session length
            return Err(SessionIdentityError::EncodingError)?;
        }

        let (session_id_b64, authentication_tag_b64) = raw_cookie_val.split_at(22);

        let authentication_tag_bytes = B64
            .decode(authentication_tag_b64)
            .map_err(|_| SessionIdentityError::EncodingError)?;

        let ecdsa_signature = ecdsa::Signature::try_from(authentication_tag_bytes.as_slice())
            .map_err(SessionIdentityError::InvalidSignatureBytes)?;
        let mut digest = hmac_sha512::sha384::Hash::new();
        digest.update(session_id_b64);

        let verification_key = ServiceVerificationKey::from_ref(state);
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

        let session_id_bytes: [u8; 16] = session_id_bytes
            .try_into()
            .expect("signed session ID to be valid byte slice");
        let session_id = SessionId::from(Uuid::from_bytes_le(session_id_bytes));

        let database = Database::from_ref(state);
        let mut conn = database
            .acquire()
            .await
            .map_err(SessionIdentityError::DatabaseConnection)?;

        let maybe_db_session = Session::locate(&mut conn, session_id)
            .await
            .map_err(SessionIdentityError::LookupFailed)?;

        let db_session = match maybe_db_session {
            Some(ds) => ds,
            None => {
                return Err(SessionIdentityError::NoMatchingSession);
            }
        };

        // todo: check session against client IP address and user agent

        if db_session.expires_at() <= OffsetDateTime::now_utc() {
            return Err(SessionIdentityError::SessionExpired);
        }

        Ok(SessionIdentity {
            id: db_session.id(),
            provider_account_id: db_session.oauth_provider_account_id(),
            user_id: db_session.user_id(),

            created_at: db_session.created_at(),
            expires_at: db_session.expires_at(),
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SessionIdentityError {
    #[error("signature did not match digest, tampering likely: {0}")]
    BadSignature(ecdsa::Error),

    #[error("received cookie that was larger than we expect or accept")]
    CookieTooLarge,

    #[error("a UUID in the database was corrupted and can not be parsed")]
    CorruptDatabaseId(uuid::Error),

    #[error("issue with database connection: {0}")]
    DatabaseConnection(sqlx::Error),

    #[error("cookie was not encoded into the correct format")]
    EncodingError,

    #[error("authenicated signature was in a valid format: {0}")]
    InvalidSignatureBytes(ecdsa::Error),

    #[error("unable to lookup session in database: {0}")]
    LookupFailed(sqlx::Error),

    #[error("received valid authorization token, but did not find matching one in the database. revocation?")]
    NoMatchingSession,

    #[error("user didn't have an existing session")]
    NoSession(String),

    #[error("session was expired")]
    SessionExpired,
}

impl IntoResponse for SessionIdentityError {
    fn into_response(self) -> Response {
        use SessionIdentityError as SIE;

        let mut cookie_jar = CookieJar::default();

        cookie_jar = remove_cookie(SESSION_COOKIE_NAME, cookie_jar);

        match self {
            SIE::NoSession(_orig_uri) => {
                tracing::debug!("request had no session when trying to access protected path");
            }
            err => tracing::warn!("session validation error: {err}"),
        }

        (cookie_jar, Redirect::to(LOGIN_PATH)).into_response()
    }
}
