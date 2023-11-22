use askama::Template;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use http::{HeaderValue, StatusCode};

use crate::app::AppState;
use crate::extractors::SessionIdentity;

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/css/metrics.css", get(css_metrics_handler))
        .route("/", get(home_handler))
        .with_state(state)
}

pub async fn home_handler(session: SessionIdentity) -> Response {
    HomeTemplate { session }.into_response()
}

pub async fn css_metrics_handler() -> Response {
    let mut headers = axum::http::HeaderMap::new();

    headers.insert(
        axum::http::header::CONTENT_TYPE,
        HeaderValue::from_static("text/css"),
    );

    let contents = "body:hover { border-image: url('/metrics/css_hit/?ref={{ request.META.HTTP_REFERER }}'); }";

    (StatusCode::OK, headers, contents).into_response()
}

#[derive(Template)]
#[template(path = "home.html")]
pub struct HomeTemplate {
    pub session: SessionIdentity,
}

#[derive(Template)]
#[template(path = "not_found.html")]
pub struct NotFoundTemplate;
