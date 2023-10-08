use std::time::Duration;

use time::OffsetDateTime;

use crate::auth::SESSION_TTL;
use crate::database::custom_types::{LoginProvider, SessionId, UserId};
use crate::database::Database;

pub struct CreateSession {
    user_id: UserId,
    provider: LoginProvider,
    access_token: String,

    access_expires_at: Option<OffsetDateTime>,
    refresh_token: Option<String>,

    client_ip: Option<String>,
    user_agent: Option<String>,
}

impl CreateSession {
    pub fn new(user_id: UserId, provider: LoginProvider, access_token: String) -> Self {
        Self {
            user_id,
            provider,
            access_token,

            access_expires_at: None,
            refresh_token: None,

            client_ip: None,
            user_agent: None,
        }
    }

    pub async fn save(self, database: &Database) -> Result<SessionId, SessionError> {
        let expires_at = OffsetDateTime::now_utc() + Duration::from_secs(SESSION_TTL);

        //let new_sid_row = sqlx::query!(
        //    r#"INSERT INTO sessions
        //        (user_id, provider, access_token, access_expires_at, refresh_token, expires_at)
        //        VALUES ($1, $2, $3, $4, $5, $6)
        //        RETURNING id;"#,
        //    user_id,
        //    provider,
        //    access_token,
        //    access_expires_at,
        //    refresh_token,
        //    expires_at,
        //)
        //.fetch_one(&database)
        //.await
        //.map_err(AuthenticationError::SessionSaveFailed)?;

        todo!()
    }
}

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
    pub async fn delete(database: &Database, id: SessionId) -> Result<(), sqlx::Error> {
        sqlx::query!("DELETE FROM sessions WHERE id = $1;", id)
            .execute(database)
            .await?;

        Ok(())
    }

    pub async fn locate(database: &Database, id: SessionId) -> Result<Option<Self>, sqlx::Error> {
        let query_result = sqlx::query_as!(Self, "SELECT * FROM sessions WHERE id = $1;", id,)
            .fetch_one(database)
            .await;

        match query_result {
            Ok(sess) => Ok(Some(sess)),
            Err(sqlx::Error::RowNotFound) => Ok(None),
            Err(err) => Err(err),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
}
