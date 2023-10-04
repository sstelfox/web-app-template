use axum::extract::{Host, Path, Query, State};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::get;
use axum::{Router};
use axum_extra::extract::cookie::Cookie;
use axum_extra::extract::CookieJar;

use oauth2::basic::BasicClient;
use oauth2::{AuthorizationCode, CsrfToken, PkceCodeVerifier, RedirectUrl, TokenResponse};
use serde::Deserialize;
use url::Url;

use crate::app::{Secrets, State as AppState};
use crate::database::Database;
use crate::extractors::{SessionIdentity, LOGIN_PATH, SESSION_COOKIE_NAME};

mod authentication_error;
mod login;
mod provider_config;

use authentication_error::AuthenticationError;
use provider_config::ProviderConfig;

static CALLBACK_PATH_TEMPLATE: &str = "/auth/callback/{}";

static PROVIDER_CONFIGS: phf::Map<&'static str, ProviderConfig> = phf::phf_map! {
    "google" => ProviderConfig::new(
        "https://accounts.google.com/o/oauth2/v2/auth",
        Some("https://www.googleapis.com/oauth2/v3/token"),
        Some("https://oauth2.googleapis.com/revoke"),
        &[
            "https://www.googleapis.com/auth/userinfo.email",
            "https://www.googleapis.com/auth/userinfo.profile"
        ],
    ),
};

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/callback/:provider", get(oauth_callback))
        .route("/login", get(select_provider_handler))
        .route("/login/:provider", get(login::handler))
        .route("/logout", get(logout_handler))
        .with_state(state)
}


pub async fn logout_handler(
    session: Option<SessionIdentity>,
    database: Database,
    mut cookie_jar: CookieJar,
) -> Response {
    if let Some(sid) = session {
        let session_id = sid.session_id();

        // todo: revoke token?

        let query = sqlx::query!("DELETE FROM sessions WHERE id = ?;", session_id);
        if let Err(err) = query.execute(&database).await {
            tracing::error!("failed to remove session from the db: {err}");
        }
    }

    cookie_jar = cookie_jar.remove(Cookie::named(SESSION_COOKIE_NAME));
    (cookie_jar, Redirect::to(LOGIN_PATH)).into_response()
}

pub async fn oauth_callback(
    database: Database,
    cookie_jar: CookieJar,
    State(state): State<AppState>,
    Host(hostname): Host,
    Path(provider): Path<String>,
    Query(params): Query<CallbackParameters>,
) -> Result<Response, AuthenticationError> {
    let csrf_secret = CsrfToken::new(params.state);
    let exchange_code = AuthorizationCode::new(params.code);

    let query_secret = csrf_secret.secret();
    let oauth_state_query: (String, Option<String>) = sqlx::query_as(
            "SELECT pkce_verifier_secret,next_url FROM oauth_state WHERE csrf_secret = ?;"
        )
        .bind(query_secret)
        .fetch_one(&database)
        .await
        .map_err(AuthenticationError::MissingCallbackState)?;

    sqlx::query!("DELETE FROM oauth_state WHERE csrf_secret = ?;", query_secret)
        .execute(&database)
        .await
        .map_err(|_| AuthenticationError::CleanupFailed)?;

    let (pkce_verifier_secret, next_url) = oauth_state_query;
    let pkce_code_verifier = PkceCodeVerifier::new(pkce_verifier_secret);

    let hostname = Url::parse(&hostname).expect("host to be valid");
    let oauth_client = oauth_client(&provider, hostname, state.secrets())?;

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
        &[("oauth_token", access_token)]
    ).expect("fixed format to be valid");

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

    let user_row = sqlx::query!("SELECT id FROM users WHERE email = LOWER($1);", user_info.email)
        .fetch_optional(&database)
        .await
        .map_err(AuthenticationError::LookupFailed)?;

    let _existing_user: Option<String> = match user_row {
        Some(u) => Some(u.id),
        None => None,
    };

    // todo:
    //  * find or create a new user account for the email
    //  * create a new session for the user
    //    * record it in the database
    //    * build and sign an appropriate cookie for it

    let redirect_url = next_url.unwrap_or("/".to_string());
    Ok((cookie_jar, Redirect::to(&redirect_url)).into_response())
}

struct MaybeId {
    id: Option<String>,
}

pub async fn select_provider_handler() -> Response {
    Html(r#"<!DOCTYPE html>
    <html>
        <head>
            <title>Select Login Provider</title>
        </head>
        <body>
            <h2>Select Login Provider:<h2>
            <ul>
                <li><a href="/auth/login/google">Login with Google</a></li>
            </ul>
        </body>
    </html>"#).into_response()
}

fn oauth_client(
    config_id: &str,
    hostname: Url,
    secrets: &Secrets,
) -> Result<BasicClient, AuthenticationError> {
    let provider_config = PROVIDER_CONFIGS
        .get(config_id)
        .ok_or(AuthenticationError::UnknownProvider)?;
    let provider_credentials = secrets
        .provider_credential(config_id)
        .ok_or(AuthenticationError::ProviderNotConfigured(config_id.to_string()))?;

    let auth_url = provider_config.auth_url();
    let token_url = provider_config.token_url();

    let mut redirect_url = hostname;
    redirect_url.set_path(&CALLBACK_PATH_TEMPLATE.replace("{}", config_id));
    let redirect_url = RedirectUrl::from_url(redirect_url);

    let mut client = BasicClient::new(
        provider_credentials.id(),
        Some(provider_credentials.secret()),
        auth_url,
        token_url,
    )
    .set_redirect_uri(redirect_url);

    if let Some(ru) = provider_config.revocation_url() {
        client = client.set_revocation_uri(ru);
    }

    Ok(client)
}

#[derive(Deserialize)]
pub struct CallbackParameters {
    code: String,
    state: String,
}

#[derive(Deserialize)]
pub struct GoogleUserProfile {
    email: String,
    verified_email: bool,
}
