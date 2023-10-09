use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Json;
use http::StatusCode;
use serde::Deserialize;

use crate::app::State as AppState;
use crate::auth::{OAuthClient, OAuthClientError};
use crate::database::custom_types::LoginProvider;
use crate::database::models::CreateOAuthState;
use crate::extractors::{ServerBase, SessionIdentity};

pub async fn handler(
    session: Option<SessionIdentity>,
    State(state): State<AppState>,
    ServerBase(hostname): ServerBase,
    Path(provider): Path<String>,
    Query(params): Query<LoginParams>,
) -> Result<Response, LoginError> {
    // already logged in, go wherever the user was originally intended or back to the root
    if session.is_some() {
        // this may be the result of a bug elsewhere improperly requiring authentication, it could
        // also indicate a phishing page is setup in front of us trying to collect authenticate
        // details
        tracing::warn!("already logged in user go directed to login handler");
        return Ok(Redirect::to(&params.next_url.unwrap_or("/".to_string())).into_response());
    }

    let provider = LoginProvider::from(provider);

    let oauth_client = OAuthClient::configure(provider, hostname, &state.secrets())
        .map_err(LoginError::UnableToConfigureOAuth)?;
    let oauth_challenge = oauth_client
        .generate_challenge()
        .await
        .map_err(LoginError::ChallengeGenerationFailed)?;
    let authorization_url = oauth_challenge.authorize_url;

    let database = state.database();
    CreateOAuthState::new(
        provider,
        oauth_challenge.csrf_token,
        oauth_challenge.pkce_code_verifier,
        params.next_url,
    )
    .save(&database)
    .await
    .map_err(LoginError::UnableToStoreSession)?;

    Ok(Redirect::to(authorization_url.as_str()).into_response())
}

#[derive(Debug, thiserror::Error)]
pub enum LoginError {
    #[error("unable to generate challenge URL for authentication: {0}")]
    ChallengeGenerationFailed(OAuthClientError),

    #[error("failed to configure OAuth client: {0}")]
    UnableToConfigureOAuth(OAuthClientError),

    #[error("unable to create session in the database: {0}")]
    UnableToStoreSession(sqlx::Error),
}

impl IntoResponse for LoginError {
    fn into_response(self) -> Response {
        tracing::error!("encountered an issue starting the login process: {self}");
        let err_msg = serde_json::json!({"msg": "backend service experienced an issue servicing the request"});
        (StatusCode::INTERNAL_SERVER_ERROR, Json(err_msg)).into_response()
    }
}

#[derive(Deserialize)]
pub struct LoginParams {
    next_url: Option<String>,
}
