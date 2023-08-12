use axum::Json;
use axum::response::{IntoResponse, Response};
use http::StatusCode;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct VersionResponse {
    build_profile: &'static str,
    features: Vec<&'static str>,
    version: &'static str,
}

pub async fn handler() -> Response {
    let msg = VersionResponse {
        build_profile: env!("BUILD_PROFILE"),
        features: env!("BUILD_FEATURES").split(',').collect::<Vec<_>>(),
        version: env!("REPO_VERSION"),
    };

    (StatusCode::OK, Json(msg)).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_handler_direct() {
        let response = handler().await;
        assert_eq!(response.status(), StatusCode::OK);

        let resp: VersionResponse = response.json().expect("parseable response");

        // todo: test the contents at least a little bit...
    }
}
