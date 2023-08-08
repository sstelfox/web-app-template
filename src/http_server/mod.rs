use std::time::Duration;

use axum::error_handling::HandleErrorLayer;
use axum::extract::DefaultBodyLimit;
use axum::Router;
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

const REQUEST_TIMEOUT_SECS: u64 = 15;

const SENSITIVE_HEADERS: &[http::HeaderName] = &[
    header::AUTHORIZATION,
    header::COOKIE,
    header::PROXY_AUTHORIZATION,
    header::SET_COOKIE,
];

mod error_handlers;
mod middleware;

async fn graceful_shutdown_blocker() {
    use tokio::signal::unix;

    let mut sig_int_handler =
        unix::signal(unix::SignalKind::interrupt()).expect("to be able to install signal handler");
    let mut sig_term_handler =
        unix::signal(unix::SignalKind::terminate()).expect("to be able to install signal handler");

    // todo: need to follow k8s signal handling rules for these different signals, aka stop
    // accepting clients 
    tokio::select! {
        _ = sig_int_handler.recv() => tracing::debug!("gracefully exiting on an interrupt signal"),
        _ = sig_term_handler.recv() => tracing::debug!("gracefully exiting on an terminate signal"),
    }
}

pub async fn run(config: Config) -> Result<(), Error> {
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
        .layer(HandleErrorLayer::new(error_handlers::server_error_handler))
        .load_shed()
        .concurrency_limit(1024)
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .layer(SetSensitiveRequestHeadersLayer::from_shared(
            SENSITIVE_HEADERS.into()
        ))
        .set_x_request_id(MakeRequestUuid)
        .layer(trace_layer)
        .propagate_x_request_id()
        .layer(DefaultBodyLimit::disable())
        .layer(ValidateRequestHeaderLayer::accept("application/json"))
        .layer(SetSensitiveResponseHeadersLayer::from_shared(
            SENSITIVE_HEADERS.into()
        ));

    tracing::info!(addr = ?config.listen_addr(), "server listening");

    let state = State::from_config(&config).await;
    let root_router = Router::new()
        //.nest("/api/v1", api::router(app_state.clone()))
        //.nest("/_status", health_check::router(app_state.clone()))
        .with_state(state)
        .fallback(error_handlers::not_found_handler);

    let app = middleware_stack.service(root_router);

    Server::bind(config.listen_addr())
        .serve(app.into_make_service())
        .with_graceful_shutdown(graceful_shutdown_blocker())
        .await?;

    Ok(())
}
