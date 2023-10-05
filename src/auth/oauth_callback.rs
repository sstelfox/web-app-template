use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum_extra::extract::cookie::{Cookie, SameSite};
use axum_extra::extract::CookieJar;
use oauth2::{AuthorizationCode, CsrfToken, PkceCodeVerifier, TokenResponse};
use serde::Deserialize;
use std::time::Duration;
use url::Url;
use uuid::Uuid;

use crate::app::State as AppState;
use crate::auth::{oauth_client, AuthenticationError};
use crate::database::Database;
use crate::extractors::ServerBase;

const NEW_USER_COOKIE_NAME: &'static str = "_is_new_user";

const SESSION_TTL: u64 = 28 * 24 * 60 * 60;

pub async fn handler(
    database: Database,
    mut cookie_jar: CookieJar,
    State(state): State<AppState>,
    ServerBase(hostname): ServerBase,
    Path(provider): Path<String>,
    Query(params): Query<CallbackParameters>,
) -> Result<Response, AuthenticationError> {
    let csrf_secret = CsrfToken::new(params.state);
    let exchange_code = AuthorizationCode::new(params.code);

    let query_secret = csrf_secret.secret();
    let oauth_state_query: (String, Option<String>) = sqlx::query_as(
        "SELECT pkce_verifier_secret,next_url FROM oauth_state WHERE csrf_secret = ?;",
    )
    .bind(query_secret)
    .fetch_one(&database)
    .await
    .map_err(AuthenticationError::MissingCallbackState)?;

    sqlx::query!(
        "DELETE FROM oauth_state WHERE csrf_secret = ?;",
        query_secret
    )
    .execute(&database)
    .await
    .map_err(|_| AuthenticationError::CleanupFailed)?;

    let (pkce_verifier_secret, next_url) = oauth_state_query;
    let pkce_code_verifier = PkceCodeVerifier::new(pkce_verifier_secret);

    let oauth_client = oauth_client(&provider, hostname.clone(), state.secrets())?;

    let token_response = tokio::task::spawn_blocking(move || {
        oauth_client
            .exchange_code(exchange_code)
            .set_pkce_verifier(pkce_code_verifier)
            .request(oauth2::reqwest::http_client)
    })
    .await
    .map_err(AuthenticationError::SpawnFailure)?
    .map_err(|err| AuthenticationError::ExchangeCodeFailure(err.to_string()))?;

    let access_token = token_response.access_token().secret();

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
        .map_err(AuthenticationError::ProfileUnavailable)?;

    if !user_info.verified_email {
        return Err(AuthenticationError::UnverifiedEmail);
    }

    // We're back in provider specific land for getting information about the authenticated user,
    // todo: allow for providers other than Google here...

    let user_row = sqlx::query!(
        "SELECT id FROM users WHERE email = LOWER($1);",
        user_info.email
    )
    .fetch_optional(&database)
    .await
    .map_err(AuthenticationError::LookupFailed)?;

    let cookie_domain = hostname
        .host_str()
        .expect("built from a hostname")
        .to_string();
    let cookie_secure = hostname.scheme() == "https";

    let user_id = match user_row {
        Some(u) => Uuid::parse_str(&u.id.to_string()).expect("db ids to be valid"),
        None => {
            let new_user_row = sqlx::query!(
                r#"INSERT INTO users (email, display_name, picture, locale)
                        VALUES ($1, $2, $3, $4) RETURNING id;"#,
                user_info.email,
                user_info.name,
                user_info.picture,
                user_info.locale,
            )
            .fetch_one(&database)
            .await
            .map_err(AuthenticationError::CreationFailed)?;

            cookie_jar = cookie_jar.add(
                Cookie::build(NEW_USER_COOKIE_NAME, "yes")
                    .path("/")
                    .http_only(false)
                    .same_site(SameSite::Strict)
                    .domain(cookie_domain.clone())
                    .secure(cookie_secure)
                    .finish(),
            );

            Uuid::parse_str(&new_user_row.id).expect("db ids to be valid")
        }
    };

    let expires_at = time::OffsetDateTime::now_utc() + Duration::from_secs(SESSION_TTL);
    let db_uid = user_id.clone().to_string();

    let new_sid_row = sqlx::query!(
        "INSERT INTO sessions (user_id, expires_at) VALUES ($1, $2) RETURNING id;",
        db_uid,
        expires_at,
    )
    .fetch_one(&database)
    .await
    .map_err(AuthenticationError::SessionSaveFailed)?;
    let session_id = Uuid::parse_str(&new_sid_row.id.to_string()).expect("db ids to be valid");

    // build and sign session id into a cookie...

    let redirect_url = next_url.unwrap_or("/".to_string());
    Ok((cookie_jar, Redirect::to(&redirect_url)).into_response())
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
