use axum::response::Response;
use axum::routing::get;
use axum::Router;
use oauth2::basic::BasicClient;
use oauth2::{AuthUrl, RedirectUrl, RevocationUrl, TokenUrl};
use url::Url;

use crate::app::{Secrets, State};

static CALLBACK_PATH_TEMPLATE: &str = "/auth/callback/{}";

static PROVIDER_CONFIGS: phf::Map<&'static str, ProviderConfig> = phf::phf_map! {
    "google" => ProviderConfig {
        auth_url: "https://accounts.google.com/o/oauth2/v2/auth",
        token_url: Some("https://www.googleapis.com/oauth2/v3/token"),
        revocation_url:  Some("https://oauth2.googleapis.com/revoke"),
        scopes: &[],
    },
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

pub async fn logout_handler() -> Response {
    todo!()
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

#[derive(Debug, thiserror::Error)]
pub enum AuthenticationError<'a> {
    #[error("no credentials available for provider '{0}'")]
    ProviderNotConfigured(&'a str),

    #[error("attempted to authenticate against an unknown provider")]
    UnknownProvider,
}

struct ProviderConfig {
    auth_url: &'static str,
    token_url: Option<&'static str>,
    revocation_url: Option<&'static str>,
    scopes: &'static [&'static str],
}

impl ProviderConfig {
    pub fn auth_url(&self) -> AuthUrl {
        AuthUrl::new(self.auth_url.to_string()).expect("static auth url to be valid")
    }

    pub fn revocation_url(&self) -> Option<RevocationUrl> {
        self.revocation_url.map(|ru| { RevocationUrl::new(ru.to_string()).expect("static revocation url to be valid") })
    }

    pub fn scopes(&self) -> &'static [&'static str] {
        self.scopes
    }

    pub fn token_url(&self) -> Option<TokenUrl> {
        self.token_url.map(|tu| { TokenUrl::new(tu.to_string()).expect("static token url to be valid") })
    }
}
