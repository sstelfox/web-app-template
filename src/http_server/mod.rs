use std::net::SocketAddr;
use std::time::Duration;

use axum::error_handling::HandleErrorLayer;
use axum::extract::DefaultBodyLimit;
use axum::handler::HandlerWithoutStateExt;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use axum::ServiceExt;
use bincode::Options;
use http::uri::PathAndQuery;
use http::{header, Request};
use time::OffsetDateTime;
use tokio::sync::watch;
use tower::ServiceBuilder;
use tower_http::request_id::MakeRequestUuid;
use tower_http::sensitive_headers::{
    SetSensitiveRequestHeadersLayer, SetSensitiveResponseHeadersLayer,
};
use tower_http::services::ServeDir;
use tower_http::trace::{DefaultOnFailure, DefaultOnResponse, MakeSpan, TraceLayer};
use tower_http::validate_request::ValidateRequestHeaderLayer;
use tower_http::{LatencyUnit, ServiceBuilderExt};
use tracing::{Level, Span};

use crate::app::{State, StateSetupError};
use crate::background_jobs::impls::TickMessage;
use crate::extractors::SessionIdentity;
use crate::{auth, health_check, pages};

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

    format!(
        "{}?{}",
        path_and_query.path(),
        filtered_query_pairs.join("&")
    )
}

pub async fn run(
    listen_addr: SocketAddr,
    log_level: Level,
    state: State,
    mut shutdown_rx: watch::Receiver<()>,
) -> Result<(), HttpServerError> {
    let trace_layer = TraceLayer::new_for_http()
        .make_span_with(SensitiveRequestMakeSpan)
        .on_response(
            DefaultOnResponse::new()
                .include_headers(false)
                .level(log_level)
                .latency_unit(LatencyUnit::Micros),
        )
        .on_failure(DefaultOnFailure::new().latency_unit(LatencyUnit::Micros));

    // todo: need to turn not_found_handler into its own service...
    let static_assets = ServeDir::new("dist")
        .precompressed_br()
        .precompressed_gzip()
        .not_found_service(error_handlers::not_found_handler.into_service());

    // todo: I think I can switch my sub-routers with different states using nest_service while
    // still having a global set of layers applied now...
    let root_router = Router::new()
        // order matters here, we inject a single dynamic asset mixed in with our static ones
        .route("/assets/css/metrics.css", get(pages::css_metrics_handler))
        .nest_service("/assets", static_assets)
        .nest("/auth", auth::router(state.clone()))
        //.nest("/api/v1", api::router(app_state.clone()))
        .nest("/_status", health_check::router(state.clone()))
        .route("/events", get(event_bus_handler))
        .route("/events/test", get(test_event_handler))
        .nest("/", pages::router(state.clone()))
        .with_state(state)
        .fallback(error_handlers::not_found_handler)
        // The order of these layers and configuration extensions was carefully chosen as they will see
        // the requests to responses effectively in the order they're defined.
        //
        // Tracing and log handling get setup before anything else
        .layer(trace_layer)
        //.layer(HandleErrorLayer::new(error_handlers::server_error_handler))
        // From here on out our requests might be logged, ensure any sensitive headers are stripped
        // before we do any logging
        .layer(SetSensitiveRequestHeadersLayer::from_shared(
            SENSITIVE_HEADERS.into(),
        ))
        // If requests are queued or take longer than this duration we want the cut them off
        // regardless of any other protections that are inplace
        // todo
        //.timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        // If any future services or middleware indicate they're not available, reject them with a
        // service too busy error
        // todo
        //.load_shed()
        // Restrict the number of concurrent in flight requests, desired value for this is going to
        // vary from service to service, make sure it reflects the number of concurrent requests
        // your service can handle.
        // todo
        //.concurrency_limit(1024)
        // Make sure our request has a unique identifier if we don't already have one. This does
        // allow our upstream to arbitrarily set headers so this service should have protection
        // against arbitrary untrusted injections of this header.
        // todo
        //.set_x_request_id(MakeRequestUuid)
        // todo
        //.propagate_x_request_id()
        // By default limit any request to this size. Individual handlers can opt-out of this limit
        // if they so choose (such as an upload handler).
        .layer(DefaultBodyLimit::max(REQUEST_MAX_SIZE))
        // Our clients should only ever be sending us JSON requests, any other type is an error.
        // This won't be true of all APIs and this will accept the wildcards sent by most clients.
        // Debatable whether I actually want this...
        // todo
        //.layer(ValidateRequestHeaderLayer::accept("application/json"))
        // Finally make sure any responses successfully generated from our service is also
        // filtering out any sensitive headers from our logs.
        .layer(SetSensitiveResponseHeadersLayer::from_shared(
            SENSITIVE_HEADERS.into(),
        ));

    tracing::info!(addr = ?listen_addr, "server listening");
    let listener = tokio::net::TcpListener::bind(listen_addr).await?;

    axum::serve(listener, root_router)
        .with_graceful_shutdown(async move {
            let _ = shutdown_rx.changed().await;
        })
        .await?;

    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum HttpServerError {
    #[error("an error occurred running the HTTP server: {0}")]
    ServingFailed(#[from] std::io::Error),

    #[error("state initialization failed: {0}")]
    StateInitializationFailed(#[from] StateSetupError),
}

use crate::event_bus::{SystemEvent, TestEvent};
use axum::http::StatusCode;

async fn test_event_handler(
    session: SessionIdentity,
    axum::extract::State(state): axum::extract::State<State>,
) -> Response {
    let _ = state.event_bus().send(
        SystemEvent::TestEvent,
        &TestEvent {
            session_id: session.id(),
        },
    );
    (StatusCode::NO_CONTENT, ()).into_response()
}

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use futures::{SinkExt, StreamExt};
use serde::Serialize;

async fn event_bus_handler(
    _session: SessionIdentity,
    upgrade_request: WebSocketUpgrade,
    axum::extract::State(state): axum::extract::State<State>,
) -> Response {
    upgrade_request.on_upgrade(|sock| event_bus_stream_handler(sock, state))
}

use crate::event_bus::UserRegistration;

async fn event_bus_stream_handler(stream: WebSocket, state: State) {
    let (mut client_tx, mut client_rx) = stream.split();

    let event_bus = state.event_bus();
    let mut bus_rx = event_bus.subscribe();

    // todo: need to force disconnects if a session is invalidated
    // todo: need to force disconnect is a session expires

    let mut bus_to_client_task = tokio::spawn(async move {
        loop {
            let (event_type, payload) = match bus_rx.recv().await {
                Ok(msg) => msg,
                Err(err) => {
                    tracing::error!("encountered bus error in websocket handling: {err}");
                    break;
                }
            };

            let bin_code_config = bincode::DefaultOptions::new();

            let decoded = match &event_type {
                SystemEvent::UserRegistration => {
                    match bin_code_config.deserialize::<UserRegistration>(&payload) {
                        Ok(event) => serde_json::to_value(&event).ok(),
                        Err(err) => {
                            tracing::warn!(
                                "failed to decode user registration on event bus: {err}"
                            );
                            None
                        }
                    }
                }
                SystemEvent::TestEvent => {
                    match bin_code_config.deserialize::<TestEvent>(&payload) {
                        Ok(event) => serde_json::to_value(&event).ok(),
                        Err(err) => {
                            tracing::warn!(
                                "failed to decode user registration on event bus: {err}"
                            );
                            None
                        }
                    }
                }
                SystemEvent::Tick => match bin_code_config.deserialize::<TickMessage>(&payload) {
                    Ok(event) => serde_json::to_value(&ClientTick::from(event)).ok(),
                    Err(err) => {
                        tracing::warn!("failed to decode tick on event bus: {err}");
                        None
                    }
                },
            };

            let response = BusToClientMessage {
                event_type,
                payload,
                decoded,
            };

            let response_msg = match serde_json::to_string(&response) {
                Ok(rm) => rm,
                Err(err) => {
                    tracing::error!("failed to serialize message to websocket client: {err}");
                    break;
                }
            };

            if let Err(err) = client_tx.send(Message::Text(response_msg)).await {
                tracing::error!("failed to send message to websocket client: {err}");
                break;
            }
        }
    });

    let mut client_to_bus_task = tokio::spawn(async move {
        while let Some(maybe_client_msg) = client_rx.next().await {
            match maybe_client_msg {
                Ok(ws_msg) => {
                    match ws_msg {
                        // Clean shutdown, there is a reason field in here that might be useful in
                        // the future though
                        Message::Close(_close_frame) => {
                            break;
                        }
                        _ => {
                            tracing::warn!("received unexpected client message: {ws_msg:?}");
                        }
                    }
                }
                Err(err) => {
                    tracing::error!("failed to receive message from client: {err}");
                    break;
                }
            }
        }
    });

    tokio::select! {
        _ = (&mut bus_to_client_task) => client_to_bus_task.abort(),
        _ = (&mut client_to_bus_task) => bus_to_client_task.abort(),
    };
}

#[derive(Serialize)]
struct BusToClientMessage {
    event_type: SystemEvent,
    payload: Vec<u8>,

    #[serde(skip_serializing_if = "Option::is_none")]
    decoded: Option<serde_json::Value>,
}

/// We use a different encoding of time when communicating outside of our applications as the byte
/// serialized version is more efficient but not a standard others want to interact with.
#[derive(Serialize)]
struct ClientTick {
    #[serde(with = "time::serde::rfc3339")]
    time: OffsetDateTime,
}

impl From<TickMessage> for ClientTick {
    fn from(value: TickMessage) -> Self {
        Self { time: value.time() }
    }
}
