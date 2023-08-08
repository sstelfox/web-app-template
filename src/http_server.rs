use std::sync::Arc;
use std::time::Duration;

use axum::error_handling::HandleErrorLayer;
use axum::extract::DefaultBodyLimit;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Json, Router};
use axum::{Server, ServiceExt};
use http::header;
use tower::ServiceBuilder;
use tower_http::request_id::MakeRequestUuid;
use tower_http::sensitive_headers::{
    SetSensitiveRequestHeadersLayer, SetSensitiveResponseHeadersLayer,
};
use tower_http::trace::{DefaultMakeSpan, DefaultOnFailure, DefaultOnResponse, TraceLayer};
use tower_http::validate_request::ValidateRequestHeaderLayer;
use tower_http::{LatencyUnit, ServiceBuilderExt};
use tracing::Level;

use crate::app::{Config, Error, State};

// todo: might want a longer timeout in some parts of the API and I'd like to be able customize a
// few layers eventually such as CORS and request timeouts but that's for something down the line
const REQUEST_TIMEOUT_SECS: u64 = 15;

async fn graceful_shutdown_blocker() {
    use tokio::signal::unix;

    let mut sig_int_handler =
        unix::signal(unix::SignalKind::interrupt()).expect("to be able to install signal handler");
    let mut sig_term_handler =
        unix::signal(unix::SignalKind::terminate()).expect("to be able to install signal handler");

    // todo: need to follow k8s signal handling rules for these different signals
    tokio::select! {
        _ = sig_int_handler.recv() => tracing::debug!("gracefully exiting on an interrupt signal"),
        _ = sig_term_handler.recv() => tracing::debug!("gracefully exiting on an terminate signal"),
    }
}

async fn handle_error(error: tower::BoxError) -> Response {
    if error.is::<tower::timeout::error::Elapsed>() {
        let msg = serde_json::json!({"status": "error", "message": "request timed out"});
        return (StatusCode::REQUEST_TIMEOUT, Json(msg)).into_response();
    }

    if error.is::<tower::load_shed::error::Overloaded>() {
        let msg = serde_json::json!({"status": "error", "message": "service overloaded"});
        return (StatusCode::SERVICE_UNAVAILABLE, Json(msg)).into_response();
    }

    let mut errors = vec![error.to_string()];
    let mut source = error.source();

    while let Some(inner_err) = source {
        errors.push(inner_err.to_string());
        source = inner_err.source();
    }

    tracing::error!(errors = ?errors, "unhandled error");

    let msg = serde_json::json!({"status": "error", "message": "unknown server error"});
    (StatusCode::INTERNAL_SERVER_ERROR, Json(msg)).into_response()
}

async fn not_found_handler() -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({"status": "not found"})),
    ).into_response()
}

pub async fn run(config: Config) -> Result<(), Error> {
    let sensitive_headers: Arc<[_]> = Arc::new([
        header::AUTHORIZATION,
        header::COOKIE,
        header::PROXY_AUTHORIZATION,
        header::SET_COOKIE,
    ]);

    let trace_layer = TraceLayer::new_for_http()
        .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
        .on_response(
            DefaultOnResponse::new()
                .include_headers(false)
                .level(Level::INFO)
                .latency_unit(LatencyUnit::Micros),
        )
        .on_failure(DefaultOnFailure::new().latency_unit(LatencyUnit::Micros));

    let middleware_stack = ServiceBuilder::new()
        .layer(HandleErrorLayer::new(handle_error))
        .load_shed()
        .concurrency_limit(1024)
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .layer(SetSensitiveRequestHeadersLayer::from_shared(Arc::clone(
            &sensitive_headers,
        )))
        .set_x_request_id(MakeRequestUuid)
        .layer(trace_layer)
        .propagate_x_request_id()
        .layer(DefaultBodyLimit::disable())
        .layer(ValidateRequestHeaderLayer::accept("application/json"))
        .layer(SetSensitiveResponseHeadersLayer::from_shared(
            sensitive_headers,
        ));

    let state = State::from_config(&config).await;
    let root_router = Router::new()
        //.nest("/api/v1", api::router(app_state.clone()))
        //.nest("/_status", health_check::router(app_state.clone()))
        .with_state(state)
        .fallback(not_found_handler);

    let app = middleware_stack.service(root_router);

    tracing::info!(addr = ?config.listen_addr(), "server listening");

    Server::bind(config.listen_addr())
        .serve(app.into_make_service())
        .with_graceful_shutdown(graceful_shutdown_blocker())
        .await?;

    Ok(())
}
