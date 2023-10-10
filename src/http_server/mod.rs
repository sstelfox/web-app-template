use std::time::Duration;

use axum::error_handling::HandleErrorLayer;
use axum::extract::DefaultBodyLimit;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use axum::{Server, ServiceExt};
use http::{header, Request};
use http::uri::PathAndQuery;
use tokio::sync::watch;
use tower::ServiceBuilder;
use tower_http::request_id::MakeRequestUuid;
use tower_http::sensitive_headers::{
    SetSensitiveRequestHeadersLayer, SetSensitiveResponseHeadersLayer,
};
use tower_http::trace::{DefaultOnFailure, DefaultOnResponse, MakeSpan, TraceLayer};
use tower_http::validate_request::ValidateRequestHeaderLayer;
use tower_http::{LatencyUnit, ServiceBuilderExt};
use tracing::{Level, Span};

use crate::app::{Config, State, StateSetupError};
use crate::extractors::SessionIdentity;
use crate::{auth, health_check};

mod error_handlers;

static FILTERED_VALUE: &str = "<filtered>";

static MISSING_VALUE: &str = "<not_provided>";

/// The largest size content that any client can send us before we reject it. This is a pretty
/// heavily restricted default but most JSON responses are relatively tiny.
const REQUEST_MAX_SIZE: usize = 256 * 1_024;

/// The maximum number of seconds that any individual request can take before it is dropped with an
/// error.
const REQUEST_TIMEOUT_SECS: u64 = 5;

const SENSITIVE_HEADERS: &[http::HeaderName] = &[
    header::AUTHORIZATION,
    header::COOKIE,
    header::PROXY_AUTHORIZATION,
    header::SET_COOKIE,
];

#[derive(Clone, Default)]
struct SensitiveRequestMakeSpan;

impl<B> MakeSpan<B> for SensitiveRequestMakeSpan {
    fn make_span(&mut self, request: &Request<B>) -> Span {
        let path_and_query = request
            .uri()
            .clone()
            .into_parts()
            .path_and_query
            .expect("http requests to have a path");

        tracing::span!(
            Level::INFO,
            "http_request",
            method = %request.method(),
            uri = %filter_path_and_query(&path_and_query),
            version = ?request.version(),
        )
    }
}

fn filter_path_and_query(path_and_query: &PathAndQuery) -> String {
    let query = match path_and_query.query() {
        Some(q) => q,
        None => {
            return path_and_query.to_string();
        }
    };

    let mut filtered_query_pairs = vec![];
    for query_pair in query.split('&') {
        let mut qp_iter = query_pair.split('=');

        match (qp_iter.next(), qp_iter.next()) {
            (Some(key), Some(val)) if !key.is_empty() && !val.is_empty() => {
                filtered_query_pairs.push([key, FILTERED_VALUE].join("="));
            }
            (Some(key), None) if !key.is_empty() => {
                filtered_query_pairs.push([key, MISSING_VALUE].join("="));
            }
            unknown => {
                tracing::warn!("encountered weird query pair: {unknown:?}");
            }
        }
    }

    if filtered_query_pairs.is_empty() {
        return path_and_query.path().to_string();
    }

    format!("{}?{}", path_and_query.path(), filtered_query_pairs.join("&"))
}

pub async fn run(config: Config, mut shutdown_rx: watch::Receiver<()>) -> Result<(), HttpServerError> {
    let trace_layer = TraceLayer::new_for_http()
        .make_span_with(SensitiveRequestMakeSpan)
        .on_response(
            DefaultOnResponse::new()
                .include_headers(false)
                .level(config.log_level())
                .latency_unit(LatencyUnit::Micros),
        )
        .on_failure(DefaultOnFailure::new().latency_unit(LatencyUnit::Micros));

    // The order of these layers and configuration extensions was carefully chosen as they will see
    // the requests to responses effectively in the order they're defined.
    let middleware_stack = ServiceBuilder::new()
        // Tracing and log handling get setup before anything else
        .layer(trace_layer)
        .layer(HandleErrorLayer::new(error_handlers::server_error_handler))
        // From here on out our requests might be logged, ensure any sensitive headers are stripped
        // before we do any logging
        .layer(SetSensitiveRequestHeadersLayer::from_shared(
            SENSITIVE_HEADERS.into(),
        ))
        // If requests are queued or take longer than this duration we want the cut them off
        // regardless of any other protections that are inplace
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        // If any future services or middleware indicate they're not available, reject them with a
        // service too busy error
        .load_shed()
        // Restrict the number of concurrent in flight requests, desired value for this is going to
        // vary from service to service, make sure it reflects the number of concurrent requests
        // your service can handle.
        .concurrency_limit(1024)
        // Make sure our request has a unique identifier if we don't already have one. This does
        // allow our upstream to arbitrarily set headers so this service should have protection
        // against arbitrary untrusted injections of this header.
        .set_x_request_id(MakeRequestUuid)
        .propagate_x_request_id()
        // By default limit any request to this size. Individual handlers can opt-out of this limit
        // if they so choose (such as an upload handler).
        .layer(DefaultBodyLimit::max(REQUEST_MAX_SIZE))
        // Our clients should only ever be sending us JSON requests, any other type is an error.
        // This won't be true of all APIs and this will accept the wildcards sent by most clients.
        // Debatable whether I actually want this...
        .layer(ValidateRequestHeaderLayer::accept("application/json"))
        // Finally make sure any responses successfully generated from our service is also
        // filtering out any sensitive headers from our logs.
        .layer(SetSensitiveResponseHeadersLayer::from_shared(
            SENSITIVE_HEADERS.into(),
        ));

    let state = State::from_config(&config).await?;
    let root_router = Router::new()
        .nest("/auth", auth::router(state.clone()))
        //.nest("/api/v1", api::router(app_state.clone()))
        .nest("/_status", health_check::router(state.clone()))
        .route("/", get(home_handler))
        .with_state(state)
        .fallback(error_handlers::not_found_handler);
    let app = middleware_stack.service(root_router);

    tracing::info!(addr = ?config.listen_addr(), "server listening");
    Server::bind(config.listen_addr())
        .serve(app.into_make_service())
        .with_graceful_shutdown(async move { let _ = shutdown_rx.changed().await; })
        .await
        .map_err(HttpServerError::ServingFailed)?;

    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum HttpServerError {
    #[error("an error occurred running the HTTP server: {0}")]
    ServingFailed(#[from] hyper::Error),

    #[error("state initialization failed: {0}")]
    StateInitializationFailed(#[from] StateSetupError),
}

pub async fn home_handler(session_id: SessionIdentity) -> Response {
    axum::response::Html(format!(
        r#"<!DOCTYPE html>
           <html>
             <head>
               <title>Home</title>
             </head>
             <body style="background: #131313; color: #9f9f9f;">
                <p>User ID: {}, Session ID: {}</p>
             </body>
           </html>"#,
        session_id.user_id(),
        session_id.session_id(),
    ))
    .into_response()
}
