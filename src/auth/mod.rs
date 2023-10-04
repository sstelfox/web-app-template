use std::collections::HashMap;

use axum::extract::{Host, Path, Query, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use axum::{Json, Router};
use axum_extra::extract::cookie::Cookie;
use axum_extra::extract::CookieJar;
use http::StatusCode;
use oauth2::basic::BasicClient;
use oauth2::RedirectUrl;
use serde::Deserialize;
use url::Url;

use crate::app::{Secrets, State as AppState};
use crate::database::Database;
use crate::extractors::{SessionIdentity, LOGIN_PATH, SESSION_COOKIE_NAME};

mod authentication_error;
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
        .with_state(state)
        .route("/login/:provider", get(login_handler))
        .route("/auth/callback/:provider", get(oauth_callback))
        .route("/logout", get(logout_handler))
}

#[axum::debug_handler]
pub async fn login_handler(
    session: Option<SessionIdentity>,
    State(state): State<AppState>,
    cookie_jar: CookieJar,
    Host(hostname): Host,
    Path(provider): Path<String>,
    Query(params): Query<LoginParams>,
) -> Response {
    let next_url = params.next_url.unwrap_or("/".to_string());
    if session.is_some() {
        return Redirect::to(&next_url).into_response();
    }

    // todo: should return an error here
    //let hostname = Url::parse(&hostname).expect("host to be valid");
    //let oauth_client = match oauth_client(&provider, hostname, &secrets) {
    //    Ok(oc) => oc,
    //    Err(err) => {
    //        let response = serde_json::json!({"msg": "unable to use login services"});
    //        return (StatusCode::INTERNAL_SERVER_ERROR, Json(response)).into_response();
    //    }
    //};

    todo!()
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

pub async fn oauth_callback() -> Response {
    todo!()
}

fn oauth_client<'a>(
    config_id: &'a str,
    hostname: Url,
    secrets: &'a Secrets,
) -> Result<BasicClient, AuthenticationError<'a>> {
    let provider_config = PROVIDER_CONFIGS
        .get(config_id)
        .ok_or(AuthenticationError::UnknownProvider)?;
    let provider_credentials = secrets
        .provider_credential(config_id)
        .ok_or(AuthenticationError::ProviderNotConfigured(config_id))?;

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
pub struct LoginParams {
    next_url: Option<String>,
}
