use axum::response::{IntoResponse, Redirect, Response};
use axum_extra::extract::CookieJar;

use crate::auth::{LOGIN_PATH, SESSION_COOKIE_NAME};
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
        if let Err(err) = Session::delete(&database, sid.id()).await {
            tracing::error!("failed to remove session from the db: {err}");
        }
    }

    cookie_jar = remove_cookie(SESSION_COOKIE_NAME, cookie_jar);
    (cookie_jar, Redirect::to(LOGIN_PATH)).into_response()
}
