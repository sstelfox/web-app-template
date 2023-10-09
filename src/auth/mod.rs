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

pub const NEW_USER_COOKIE_NAME: &str = "_is_new_user";

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
    Html(
        r#"<!DOCTYPE html>
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
    </html>"#,
    )
    .into_response()
}
