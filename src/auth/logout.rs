use axum::response::{IntoResponse, Redirect, Response};
use axum_extra::extract::cookie::Cookie;
use axum_extra::extract::CookieJar;

use crate::database::Database;
use crate::extractors::{SessionIdentity, LOGIN_PATH, SESSION_COOKIE_NAME};

pub async fn handler(
    session: Option<SessionIdentity>,
    database: Database,
    mut cookie_jar: CookieJar,
) -> Response {
    if let Some(sid) = session {
        let session_id = sid.session_id();

        // todo: revoke token?

        let query = sqlx::query!("DELETE FROM sessions WHERE id = ?;", session_id);
        if let Err(err) = query.execute(&database).await {
            tracing::error!("failed to remove session from the db: {err}");
        }
    }

    cookie_jar = cookie_jar.remove(Cookie::named(SESSION_COOKIE_NAME));
    (cookie_jar, Redirect::to(LOGIN_PATH)).into_response()
}
