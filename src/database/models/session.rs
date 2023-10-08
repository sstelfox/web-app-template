use time::OffsetDateTime;

use crate::database::Database;
use crate::database::custom_types::{LoginProvider, SessionId, UserId};

#[derive(sqlx::FromRow)]
pub struct Session {
    id: SessionId,
    user_id: UserId,

    provider: LoginProvider,
    access_token: String,
    access_expires_at: Option<OffsetDateTime>,
    refresh_token: Option<String>,

    client_ip: Option<String>,
    user_agent: Option<String>,

    created_at: OffsetDateTime,
    expires_at: Option<OffsetDateTime>,
}

impl Session {
    pub async fn locate(database: &Database, id: SessionId) -> Result<Option<Self>, sqlx::Error> {
        let query_result = sqlx::query_as!(
            Self,
            "SELECT * FROM sessions WHERE id = $1;",
            id,
        )
        .fetch_one(database)
        .await;

        match query_result {
            Ok(sess) => Ok(Some(sess)),
            Err(sqlx::Error::RowNotFound) => Ok(None),
            Err(err) => Err(err),
        }
    }
}
