use askama::Template;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use http::{HeaderValue, StatusCode};

use crate::app::AppState;
use crate::extractors::{Requestor, SessionIdentity};

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", get(home_handler))
        .with_state(state)
}

pub async fn home_handler(session: SessionIdentity) -> Response {
    HomeTemplate { session }.into_response()
}

pub async fn css_metrics_handler(requestor: Requestor) -> Response {
    if requestor.is_private() {
        return (StatusCode::NO_CONTENT, ()).into_response();
    }

    let mut headers = axum::http::HeaderMap::new();

    headers.insert(
        axum::http::header::CONTENT_TYPE,
        HeaderValue::from_static("text/css"),
    );

    // todo: probably want to do something a bit more creative here, deflate + bate64, maybe a
    // structured value...
    let query_str = match requestor.referrer() {
        Some(referrer) => format!("?ref={}", referrer),
        None => "".to_string(),
    };

    let contents = format!("body:hover {{ border-image: url('/metrics/css_hit/{query_str}'); }}");
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
