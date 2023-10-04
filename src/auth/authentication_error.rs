use axum::Json;
use axum::response::{IntoResponse, Response};
use http::StatusCode;

#[derive(Debug, thiserror::Error)]
pub enum AuthenticationError {
    #[error("failed to clean up intermediate session state")]
    CleanupFailed,

    #[error("received callback from oauth but we didn't have a matching session")]
    MissingCallbackState(sqlx::Error),

    #[error("failed to build oauth client: {0}")]
    OAuthClientUnavailable(String),

    #[error("no credentials available for provider '{0}'")]
    ProviderNotConfigured(String),

    #[error("attempted to authenticate against an unknown provider")]
    UnknownProvider,
}

impl IntoResponse for AuthenticationError {
    fn into_response(self) -> Response {
        use AuthenticationError as AE;

        match self {
            AE::CleanupFailed | AE::OAuthClientUnavailable(_) => {
                tracing::error!("{}", &self);
                let msg = serde_json::json!({"msg": "authentication workflow broke down"});
                (StatusCode::INTERNAL_SERVER_ERROR, Json(msg)).into_response()
            },
            AE::MissingCallbackState(ref err) => {
                tracing::warn!("{}: {err}", &self);
                let msg = serde_json::json!({"msg": "unknown authentication callback"});
                (StatusCode::BAD_REQUEST, Json(msg)).into_response()
            },
            AE::ProviderNotConfigured(_) | AE::UnknownProvider => {
                tracing::warn!("{}", &self);
                let msg = serde_json::json!({"msg": "unknown provider or provider not configured"});
                (StatusCode::NOT_FOUND, Json(msg)).into_response()
            },
        }
    }
}
