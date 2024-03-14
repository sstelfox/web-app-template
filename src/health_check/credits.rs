use axum::response::{IntoResponse, Response};
use axum::Json;
use http::StatusCode;

pub async fn handler() -> Response {
    // todo: add in additional core projects into this list
    let credits = serde_json::json!([
        {
            "name": "FontAwesome",
            "description": "icon library typeface",
            "site": "https://fontawesome.com/",
            "version": "6.5.1",
        },
        {
            "name": "Inter",
            "description": "flexible typeface used in the web interface",
            "site": "https://rsms.me/inter/",
            "version": "4.0",
        },
    ]);

    (StatusCode::OK, Json(credits)).into_response()
}
