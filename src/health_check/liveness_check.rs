use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

pub async fn handler() -> Response {
    let msg = serde_json::json!({"status": "ok"});
    (StatusCode::OK, Json(msg)).into_response()
}
