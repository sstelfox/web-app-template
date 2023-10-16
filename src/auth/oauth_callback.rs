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


use url::Url;

use crate::app::State as AppState;
use crate::auth::{OAuthClient, OAuthClientError};
use crate::database::models::{
    CreateOAuthProviderAccount, CreateSession, CreateUser, OAuthStateError, SessionError, UserError, VerifyOAuthState,
};
use crate::auth::{SESSION_COOKIE_NAME};
use crate::database::custom_types::{LoginProvider, OAuthProviderAccountId, OAuthProviderAccountIdError, ProviderId, UserId, UserIdError};
use crate::database::models::{OAuthProviderAccount, OAuthProviderAccountError};
use crate::database::Database;
use crate::extractors::ServerBase;

pub async fn handler(
    database: Database,
    mut cookie_jar: CookieJar,
    State(state): State<AppState>,
    ServerBase(hostname): ServerBase,
    Path(provider): Path<LoginProvider>,
    Query(params): Query<CallbackParameters>,
) -> Result<Response, OAuthCallbackError> {
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

    let maybe_provider_account_id = OAuthProviderAccountId::from_provider_account_id(&database, provider, user_info.google_id.clone())
        .await
        .map_err(OAuthCallbackError::FailedAccountLookup)?;

    let provider_account_id = match maybe_provider_account_id {
        Some(pa) => pa,
        None => {
            if !user_info.verified_email {
                return Err(OAuthCallbackError::UnverifiedEmail);
            }

            let existing_user = UserId::from_email(&database, &user_info.email)
                .await
                .map_err(OAuthCallbackError::UserCheckFailed)?;

            // we need to make sure someone isn't trying to access an existing account from an
            // unknown provider claiming the same email address
            if let Some(user_id) = existing_user {
                tracing::warn!(user_id = ?user_id, "attempt to access account from unauthorized provider");
                return Err(OAuthCallbackError::AlternateProvider);
            }

            let new_user_id = CreateUser::new(user_info.email.clone(), user_info.name)
                .save(&database)
                .await
                .map_err(OAuthCallbackError::UserCreationFailed)?;

            CreateOAuthProviderAccount::new(
                    new_user_id,
                    provider,
                    user_info.google_id,
                    user_info.email.to_string(),
                )
                .save(&database)
                .await
                .map_err(OAuthCallbackError::ProviderAccountCreationFailed)?
        }
    };

    let provider_account = OAuthProviderAccount::lookup_by_id(&database, provider_account_id)
        .await
        .map_err(OAuthCallbackError::AccountDetailLookupFailed)?
        .ok_or(OAuthCallbackError::AccountIntegrityViolation)?;

    let mut new_session = CreateSession::new(provider_account.user_id(), provider_account.id());

    if let Some(access_lifetime) = token_response.expires_in() {
        new_session.limit_duration_to(access_lifetime);
    }
    let session_expires_at = new_session.expires_at();

    // todo: store client IP and user_agent in the session if they're available as well

    let session_id = new_session
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
    let session_value = [session_enc, auth_tag].join("");

    cookie_jar = cookie_jar.add(
        Cookie::build(SESSION_COOKIE_NAME, session_value)
            .http_only(true)
            .expires(session_expires_at)
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
    // This is an all numeric ID (sample one was 21 digits) that comes in as a string, probably
    // could be stored as a number but I'd rather treat it as a unique identifier.
    #[serde(rename = "id")]
    google_id: ProviderId,

    name: String,
    email: String,
    verified_email: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum OAuthCallbackError {
    #[error("account disappeared in path that guarantees its presence")]
    AccountIntegrityViolation,

    #[error("failed to load details of provider account for session creation: {0}")]
    AccountDetailLookupFailed(OAuthProviderAccountError),

    #[error("successful login from an unauthorized provider for existing account")]
    AlternateProvider,

    #[error("failed to query the database for a provider account: {0}")]
    FailedAccountLookup(OAuthProviderAccountIdError),

    #[error("unable to query OAuth states for callback parameter")]
    LookupFailed(OAuthStateError),

    #[error("failed to check whether a new user's email was present for creation: {0}")]
    UserCheckFailed(UserIdError),

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

    #[error("failed to create provider account after successful login: {0}")]
    ProviderAccountCreationFailed(OAuthProviderAccountError),

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
