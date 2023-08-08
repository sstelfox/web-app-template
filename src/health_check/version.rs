use axum::Json;
use axum::response::{IntoResponse, Response};
use http::StatusCode;

pub async fn handler() -> Response {
    let msg = serde_json::json!({
        "build_profile": env!("BUILD_PROFILE"),
        "features": env!("BUILD_FEATURES").split(',').collect::<Vec<_>>(),
        "version": env!("REPO_VERSION"),
    });

    (StatusCode::OK, Json(msg)).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    use serde::Deserialize;

    #[tokio::test]
    async fn test_handler_direct() {
        let response = handler().await;
        assert_eq!(response.status(), StatusCode::OK);

        // todo: test the contents at least a little bit...
    }
}
