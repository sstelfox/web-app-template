use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use axum::Router;
use axum_extra::extract::CookieJar;
use oauth2::RedirectUrl;
use oauth2::basic::BasicClient;
use url::Url;

use crate::app::{Secrets, State};
use crate::database::Database;
use crate::extractors::SessionIdentity;

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

pub fn router(state: State) -> Router<State> {
    Router::new()
        .route("/login", get(login_handler))
        .route("/auth/callback/:provider", get(oauth_callback))
        .route("/logout", get(logout_handler))
        .with_state(state)
}

pub async fn oauth_callback() -> Response {
    todo!()
}

pub async fn login_handler() -> Response {
    todo!()
}

pub async fn logout_handler(session: Option<SessionIdentity>, database: Database, cookie_jar: CookieJar) -> Response {
    if let Some(sid) = session {
        let session_id = sid.session_id();
        let query = sqlx::query!("DELETE FROM sessions WHERE id = ?;", session_id);

        if let Err(err) = query.execute(&database).await {
            tracing::error!("failed to remove session from the db: {err}");
        }

        // todo: revoke token?
    }

    (cookie_jar, Redirect::to("/login")).into_response()
}

fn oauth_client<'a>(config_id: &'a str, hostname: Url, secrets: &'a Secrets) -> Result<BasicClient, AuthenticationError<'a>> {
    let provider_config = PROVIDER_CONFIGS.get(config_id).ok_or(AuthenticationError::UnknownProvider)?;
    let provider_credentials = secrets.provider_credential(config_id).ok_or(AuthenticationError::ProviderNotConfigured(config_id))?;

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
