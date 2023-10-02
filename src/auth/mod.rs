use axum::http::StatusCode;
use axum::routing::get;
use axum::response::{IntoResponse, Response};
use axum::Router;

use crate::app::State;

pub fn router(state: State) -> Router<State> {
    Router::new()
        .route("/login", get(login_handler))
        .route("/oauth/callback/google", get(google_oauth_callback))
        .route("/logout", get(logout_handler))
        .with_state(state)
}

pub async fn google_oauth_callback() -> Response {
    todo!()
}

pub async fn login_handler() -> Response {
    todo!()
}

pub async fn logout_handler() -> Response {
    todo!()
}