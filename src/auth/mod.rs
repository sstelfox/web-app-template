use askama::Template;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::Router;

use crate::app::State;

mod login;
mod logout;
mod oauth_callback;
mod oauth_client;

pub use oauth_client::{OAuthClient, OAuthClientError};

pub static CALLBACK_PATH_TEMPLATE: &str = "/auth/callback/{}";

pub static LOGIN_PATH: &str = "/auth/login";

pub static SESSION_COOKIE_NAME: &str = "_session_id";

pub const SESSION_TTL: u64 = 28 * 24 * 60 * 60;

pub fn router(state: State) -> Router<State> {
    Router::new()
        .route("/callback/:provider", get(oauth_callback::handler))
        .route("/login", get(select_provider_handler))
        .route("/login/:provider", get(login::handler))
        .route("/logout", get(logout::handler))
        .with_state(state)
}

pub async fn select_provider_handler() -> Response {
    LoginTemplate.into_response()
}

#[derive(Template)]
#[template(path = "login.html")]
pub struct LoginTemplate;
