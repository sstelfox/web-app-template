use axum::Json;
use axum::response::{IntoResponse, Response};
use http::StatusCode;

use crate::app::Version;

pub async fn handler() -> Response {
    (StatusCode::OK, Json(Version::new())).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    use serde::Deserialize;

    #[tokio::test]
    async fn test_handler_direct() {
        let response = handler().await;
        assert_eq!(response.status(), StatusCode::OK);

        let resp: Version = response.json().expect("parseable response");

        // todo: test the contents at least a little bit...
    }
}
