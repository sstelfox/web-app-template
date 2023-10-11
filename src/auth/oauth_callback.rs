use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Json;
use axum_extra::extract::cookie::{Cookie, SameSite};
use axum_extra::extract::CookieJar;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
use base64::Engine;
use ecdsa::signature::RandomizedDigestSigner;
use http::StatusCode;
use jwt_simple::algorithms::ECDSAP384KeyPairLike;
use oauth2::{AuthorizationCode, CsrfToken, TokenResponse};
use serde::Deserialize;
use std::time::Duration;
use time::OffsetDateTime;
use url::Url;

use crate::app::State as AppState;
use crate::auth::{OAuthClient, OAuthClientError};
use crate::database::models::{
    CreateSession, CreateUser, OAuthStateError, SessionError, UserError, VerifyOAuthState,
};

use crate::auth::{NEW_USER_COOKIE_NAME, SESSION_COOKIE_NAME, SESSION_TTL};
use crate::database::custom_types::{LoginProvider, UserId, UserIdError};
use crate::database::Database;
use crate::extractors::ServerBase;

pub async fn handler(
    database: Database,
    mut cookie_jar: CookieJar,
    State(state): State<AppState>,
    ServerBase(hostname): ServerBase,
    Path(provider): Path<String>,
    Query(params): Query<CallbackParameters>,
) -> Result<Response, OAuthCallbackError> {
    // todo: need error
    let provider = LoginProvider::parse_str(provider.as_str()).expect("valid provider");

    let verify_oauth_state =
        VerifyOAuthState::locate_and_delete(&database, provider, params.csrf_token)
            .await
            .map_err(OAuthCallbackError::LookupFailed)?
            .ok_or(OAuthCallbackError::NoMatchingState)?;

    let oauth_client = OAuthClient::configure(provider, hostname.clone(), &state.secrets())
        .map_err(OAuthCallbackError::UnableToConfigureOAuth)?;

    let pkce_code_verifier = verify_oauth_state.pkce_code_verifier();
    let token_response = tokio::task::spawn_blocking(move || {
        oauth_client.validate_exchange(params.authorization_code, pkce_code_verifier)
    })
    .await
    .map_err(OAuthCallbackError::SpawnFailure)?
    .map_err(OAuthCallbackError::ValidationFailed)?;

    let access_token = token_response.access_token();

    let cookie_domain = hostname
        .host_str()
        .expect("built from a hostname")
        .to_string();
    let cookie_secure = hostname.scheme() == "https";

    // We're back in provider specific land for getting information about the authenticated user,
    // todo: need to abstract this somehow for different implementors...

    let user_info_url = Url::parse_with_params(
        "https://www.googleapis.com/oauth2/v2/userinfo",
        &[("oauth_token", access_token.secret())],
    )
    .expect("fixed format to be valid");

    let user_info: GoogleUserProfile = reqwest::get(user_info_url)
        .await
        .expect("building a fixed format request to succeed")
        .json()
        .await
        .map_err(OAuthCallbackError::ProfileUnavailable)?;

    // out of provider specific land for the most part

    let expires_at = OffsetDateTime::now_utc() + Duration::from_secs(SESSION_TTL);
    let maybe_user_id = UserId::from_email(&database, &user_info.email)
        .await
        .map_err(OAuthCallbackError::FailedUserLookup)?;

    let user_id = match maybe_user_id {
        Some(uid) => uid,
        None => {
            if !user_info.verified_email {
                return Err(OAuthCallbackError::UnverifiedEmail);
            }

            let mut create_user = CreateUser::new(user_info.email, user_info.name);

            create_user.locale(user_info.locale);

            match Url::parse(&user_info.picture) {
                Ok(url) => {
                    create_user.profile_image(url);
                }
                Err(err) => {
                    tracing::warn!("got invalid profile image, not storing corrupted URL: {err}");
                }
            }

            let new_user_id = create_user
                .save(&database)
                .await
                .map_err(OAuthCallbackError::UserCreationFailed)?;

            cookie_jar = cookie_jar.add(
                Cookie::build(NEW_USER_COOKIE_NAME, "yes")
                    .http_only(false)
                    .expires(expires_at)
                    .same_site(SameSite::Lax)
                    .path("/")
                    .domain(cookie_domain.clone())
                    .secure(cookie_secure)
                    .finish(),
            );

            new_user_id
        }
    };

    let mut create_session = CreateSession::new(user_id, provider, access_token.clone());

    if let Some(access_lifetime) = token_response.expires_in() {
        create_session.access_expires_at(OffsetDateTime::now_utc() + access_lifetime);
    }

    if let Some(refresh_token) = token_response.refresh_token() {
        create_session.refresh_token(refresh_token.secret().to_string());
    }

    // todo: store client IP and user_agent in the session if they're available as well

    let session_id = create_session
        .save(&database)
        .await
        .map_err(OAuthCallbackError::SessionCreationFailed)?;

    let session_enc = B64.encode(session_id.to_bytes_le());

    let mut digest = hmac_sha512::sha384::Hash::new();
    digest.update(session_enc.as_bytes());
    let mut rng = rand::thread_rng();

    let service_signing_key = state.secrets().service_signing_key();
    let signature: ecdsa::Signature<p384::NistP384> = service_signing_key
        .key_pair()
        .as_ref()
        .sign_digest_with_rng(&mut rng, digest);

    let auth_tag = B64.encode(signature.to_vec());
    tracing::info!(auth_tag = ?auth_tag, auth_tag_len = ?auth_tag.len(), "auth tag length");
    let session_value = [session_enc, auth_tag].join("");

    cookie_jar = cookie_jar.add(
        Cookie::build(SESSION_COOKIE_NAME, session_value)
            .http_only(true)
            .expires(expires_at)
            .same_site(SameSite::Lax)
            .path("/")
            .domain(cookie_domain)
            .secure(cookie_secure)
            .finish(),
    );

    let redirect_url = verify_oauth_state
        .post_login_redirect_url()
        .unwrap_or("/".to_string());

    Ok((cookie_jar, Redirect::to(&redirect_url)).into_response())
}

#[derive(Deserialize)]
pub struct CallbackParameters {
    #[serde(rename = "code")]
    authorization_code: AuthorizationCode,

    #[serde(rename = "state")]
    csrf_token: CsrfToken,
}

#[derive(Deserialize)]
pub struct GoogleUserProfile {
    name: String,
    email: String,
    verified_email: bool,

    picture: String,
    locale: String,
}

#[derive(Debug, thiserror::Error)]
pub enum OAuthCallbackError {
    #[error("failed to query the databse for a user: {0}")]
    FailedUserLookup(UserIdError),

    #[error("unable to query OAuth states for callback parameter")]
    LookupFailed(OAuthStateError),

    #[error("received OAuth callback query but no matching session parameters were present")]
    NoMatchingState,

    #[error("unable to request user's profile: {0}")]
    ProfileUnavailable(reqwest::Error),

    #[error("failed to create new session after successful login: {0}")]
    SessionCreationFailed(SessionError),

    #[error("failed to spawn blocking task for exchange code authorization: {0}")]
    SpawnFailure(tokio::task::JoinError),

    #[error("failed to configure OAuth client: {0}")]
    UnableToConfigureOAuth(OAuthClientError),

    #[error("user account must be verified before it can be used to login")]
    UnverifiedEmail,

    #[error("failed to create new user after successful login: {0}")]
    UserCreationFailed(UserError),

    #[error("failed to validate authorization code: {0}")]
    ValidationFailed(OAuthClientError),
}

impl IntoResponse for OAuthCallbackError {
    fn into_response(self) -> Response {
        match self {
            OAuthCallbackError::NoMatchingState => {
                let msg = serde_json::json!({"msg": "no matching authentication state"});
                (StatusCode::NOT_FOUND, Json(msg)).into_response()
            }
            _ => {
                tracing::error!("encountered an issue completing the login process: {self}");
                let err_msg = serde_json::json!({"msg": "backend service experienced an issue servicing the request"});
                (StatusCode::INTERNAL_SERVER_ERROR, Json(err_msg)).into_response()
            }
        }
    }
}
