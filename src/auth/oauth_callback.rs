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
use oauth2::{AuthorizationCode, CsrfToken, PkceCodeVerifier, TokenResponse};
use serde::Deserialize;
use std::time::Duration;
use time::OffsetDateTime;
use url::Url;
use uuid::Uuid;

use crate::app::State as AppState;
use crate::auth::{OAuthClient, OAuthClientError};
use crate::database::models::VerifyOAuthState;

use crate::auth::{NEW_USER_COOKIE_NAME, SESSION_COOKIE_NAME, SESSION_TTL};
use crate::database::custom_types::LoginProvider;
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
    let csrf_token = CsrfToken::new(params.state);
    let authorization_code = AuthorizationCode::new(params.code);

    let verify_oauth_state = VerifyOAuthState::locate_and_delete(&database, provider, csrf_token)
        .await
        .map_err(OAuthCallbackError::MissingCallbackState)?;

    let oauth_client = OAuthClient::configure(provider, hostname.clone(), &state.secrets())
        .map_err(OAuthCallbackError::UnableToConfigureOAuth)?;

    let post_login_redirect_url = verify_oauth_state.post_login_redirect_url();
    let pkce_code_verifier = verify_oauth_state.pkce_code_verifier();

    let token_response = tokio::task::spawn_blocking(move || {
        oauth_client.validate_exchange(authorization_code, pkce_code_verifier)
    })
    .await
    .map_err(OAuthCallbackError::SpawnFailure)?
    .map_err(OAuthCallbackError::ValidationFailed)?;

    let access_token = token_response.access_token().secret();
    let access_expires_at = token_response.expires_in().map(|secs| OffsetDateTime::now_utc() + secs);
    let refresh_token = token_response.refresh_token().map(|rt| rt.secret());

    let cookie_domain = hostname
        .host_str()
        .expect("built from a hostname")
        .to_string();
    let cookie_secure = hostname.scheme() == "https";

    // We're back in provider specific land for getting information about the authenticated user,
    // todo: need to abstract this somehow for different implementors...

    let user_info_url = Url::parse_with_params(
        "https://www.googleapis.com/oauth2/v2/userinfo",
        &[("oauth_token", access_token)],
    )
    .expect("fixed format to be valid");

    let user_info: GoogleUserProfile = reqwest::get(user_info_url)
        .await
        .expect("building a fixed format request to succeed")
        .json()
        .await
        .map_err(OAuthCallbackError::ProfileUnavailable)?;

    if !user_info.verified_email {
        return Err(OAuthCallbackError::UnverifiedEmail);
    }

    //let user_row = sqlx::query!(
    //    "SELECT id FROM users WHERE email = LOWER($1);",
    //    user_info.email
    //)
    //.fetch_optional(&database)
    //.await
    //.map_err(AuthenticationError::LookupFailed)?;

    //let user_id = match user_row {
    //    Some(u) => Uuid::parse_str(&u.id.to_string()).expect("db ids to be valid"),
    //    None => {
    //        let new_user_row = sqlx::query!(
    //            r#"INSERT INTO users (email, display_name, locale, profile_image)
    //                    VALUES (LOWER($1), $2, $3, $4) RETURNING id;"#,
    //            user_info.email,
    //            user_info.name,
    //            user_info.locale,
    //            user_info.picture,
    //        )
    //        .fetch_one(&database)
    //        .await
    //        .map_err(AuthenticationError::CreationFailed)?;

    //        cookie_jar = cookie_jar.add(
    //            Cookie::build(NEW_USER_COOKIE_NAME, "yes")
    //                .http_only(false)
    //                .expires(None)
    //                .same_site(SameSite::Lax)
    //                .domain(cookie_domain.clone())
    //                .secure(cookie_secure)
    //                .finish(),
    //        );

    //        Uuid::parse_str(&new_user_row.id).expect("db ids to be valid")
    //    }
    //};

    let expires_at = OffsetDateTime::now_utc() + Duration::from_secs(SESSION_TTL);
    //let db_uid = user_id.clone().to_string();

    //let new_sid_row = sqlx::query!(
    //    r#"INSERT INTO sessions
    //        (user_id, provider, access_token, access_expires_at, refresh_token, expires_at)
    //        VALUES ($1, $2, $3, $4, $5, $6)
    //        RETURNING id;"#,
    //    db_uid,
    //    provider,
    //    access_token,
    //    access_expires_at,
    //    refresh_token,
    //    expires_at,
    //)
    //.fetch_one(&database)
    //.await
    //.map_err(AuthenticationError::SessionSaveFailed)?;

    //let session_id = Uuid::parse_str(&new_sid_row.id.to_string()).expect("db ids to be valid");

    //let session_enc = B64.encode(session_id.to_bytes_le());
    let mut digest = hmac_sha512::sha384::Hash::new();
    //digest.update(session_enc.as_bytes());
    let mut rng = rand::thread_rng();

    let service_signing_key = state.secrets().service_signing_key();
    let signature: ecdsa::Signature<p384::NistP384> = service_signing_key
        .key_pair()
        .as_ref()
        .sign_digest_with_rng(&mut rng, digest);

    let auth_tag = B64.encode(signature.to_vec());
    //let session_value = format!("{session_enc}*{auth_tag}");

    //cookie_jar = cookie_jar.add(
    //    Cookie::build(SESSION_COOKIE_NAME, session_value)
    //        .http_only(true)
    //        .expires(expires_at)
    //        .same_site(SameSite::Lax)
    //        .path("/")
    //        .domain(cookie_domain)
    //        .secure(cookie_secure)
    //        .finish(),
    //);

    let redirect_url = post_login_redirect_url.unwrap_or("/".to_string());
    //Ok((cookie_jar, Redirect::to(&redirect_url)).into_response())

    todo!()
}

#[derive(Deserialize)]
pub struct CallbackParameters {
    code: String,
    state: String,
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
    #[error("received callback from oauth but we didn't have a matching session")]
    MissingCallbackState(sqlx::Error),

    #[error("unable to request user's profile: {0}")]
    ProfileUnavailable(reqwest::Error),

    #[error("failed to spawn blocking task for exchange code authorization: {0}")]
    SpawnFailure(tokio::task::JoinError),

    #[error("failed to configure OAuth client: {0}")]
    UnableToConfigureOAuth(OAuthClientError),

    #[error("user account must be verified before it can be used to login")]
    UnverifiedEmail,

    #[error("failed to validate authorization code: {0}")]
    ValidationFailed(OAuthClientError),
}

impl IntoResponse for OAuthCallbackError {
    fn into_response(self) -> Response {
        match self {
            OAuthCallbackError::MissingCallbackState(ref err) => {
                tracing::warn!("{}: {err}", &self);
                let msg = serde_json::json!({"msg": "unknown authentication callback"});
                (StatusCode::BAD_REQUEST, Json(msg)).into_response()
            }
            _ => {
                tracing::error!("encountered an issue completing the login process: {self}");
                let err_msg = serde_json::json!({"msg": "backend service experienced an issue servicing the request"});
                (StatusCode::INTERNAL_SERVER_ERROR, Json(err_msg)).into_response()
            }
        }
    }
}
