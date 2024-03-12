use axum::response::{IntoResponse, Redirect, Response};
use axum_extra::extract::CookieJar;

use crate::auth::{LOGIN_PATH, SESSION_COOKIE_NAME};
use crate::database::custom_types::SessionId;
use crate::database::models::Session;
use crate::database::Database;
use crate::extractors::SessionIdentity;
use crate::utils::remove_cookie;

pub async fn handler(
    session: Option<SessionIdentity>,
    database: Database,
    mut cookie_jar: CookieJar,
) -> Response {
    if let Some(sid) = session {
        try_clear_session(&database, sid.id()).await;
    }

    cookie_jar = remove_cookie(SESSION_COOKIE_NAME, cookie_jar);
    (cookie_jar, Redirect::to(LOGIN_PATH)).into_response()
}

async fn try_clear_session(database: &Database, sid: SessionId) {
    let mut conn = match database.acquire().await {
        Ok(conn) => conn,
        Err(err) => {
            tracing::warn!("failed to acquire database connection when clearing a session: {err}");
            return;
        }
    };

    if let Err(err) = Session::delete(&mut conn, sid).await {
        tracing::error!("failed to remove session from the db: {err}");
    }
}
