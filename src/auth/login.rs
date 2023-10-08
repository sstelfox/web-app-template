use axum::Json;
use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Redirect, Response};
use http::StatusCode;
use serde::Deserialize;

use crate::app::State as AppState;
use crate::auth::{OAuthClient, OAuthClientError};
use crate::database::custom_types::LoginProvider;
use crate::database::models::NewOAuthState;
use crate::extractors::{ServerBase, SessionIdentity};

pub async fn handler(
    session: Option<SessionIdentity>,
    State(state): State<AppState>,
    ServerBase(hostname): ServerBase,
    Path(provider): Path<LoginProvider>,
    Query(params): Query<LoginParams>,
) -> Result<Response, LoginError> {
    if session.is_some() {
        return Ok(Redirect::to(&params.next_url.unwrap_or("/".to_string())).into_response());
    }

    let oauth_client = OAuthClient::configure(provider, hostname, &state.secrets())
        .map_err(LoginError::UnableToConfigureOAuth)?;
    let challenge = oauth_client.generate_challenge()
        .await
        .map_err(LoginError::ChallengeGenerationFailed)?;

    let database = state.database();
    let authorization_url = challenge.authorize_url;
    let query_res = NewOAuthState::new(provider, challenge.csrf_token, challenge.pkce_code_verifier, params.next_url)
        .save(&database)
        .await;

    if let Err(err) = query_res {
        tracing::error!("failed to create oauth session handle: {err}");
        let response = serde_json::json!({"msg": "unable to use login services"});
        return Ok((StatusCode::INTERNAL_SERVER_ERROR, Json(response)).into_response());
    }

    Ok(Redirect::to(authorization_url.as_str()).into_response())
}

#[derive(Deserialize)]
pub struct LoginParams {
    next_url: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum LoginError {
    #[error("unable to generate challenge URL for authentication: {0}")]
    ChallengeGenerationFailed(OAuthClientError),

    #[error("failed to configure OAuth client: {0}")]
    UnableToConfigureOAuth(OAuthClientError),
}

impl IntoResponse for LoginError {
    fn into_response(self) -> Response {
        tracing::error!("encountered an issue starting the login process: {self}");
        let err_msg = serde_json::json!({"msg": "backend service experienced an issue servicing the request"});
        (StatusCode::INTERNAL_SERVER_ERROR, Json(err_msg)).into_response()
    }
}
