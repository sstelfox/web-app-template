use std::net::IpAddr;
use std::ops::Deref;
use std::time::Duration;

use oauth2::AccessToken;
use time::OffsetDateTime;

use crate::auth::SESSION_TTL;
use crate::database::custom_types::{LoginProvider, SessionId, UserId};
use crate::database::Database;

pub struct CreateSession {
    user_id: UserId,
    provider: LoginProvider,
    access_token: AccessToken,

    access_expires_at: Option<OffsetDateTime>,
    refresh_token: Option<String>,

    client_ip: Option<IpAddr>,
    user_agent: Option<String>,
}

impl CreateSession {
    pub fn access_expires_at(&mut self, access_expires_at: OffsetDateTime) -> &mut Self {
        self.access_expires_at = Some(access_expires_at);
        self
    }

    pub fn client_ip(&mut self, client_ip: IpAddr) -> &mut Self {
        self.client_ip = Some(client_ip);
        self
    }

    pub fn new(user_id: UserId, provider: LoginProvider, access_token: AccessToken) -> Self {
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

    pub fn refresh_token(&mut self, refresh_token: String) -> &mut Self {
        self.refresh_token = Some(refresh_token);
        self
    }

    pub async fn save(self, database: &Database) -> Result<SessionId, SessionError> {
        let client_ip_str = self.client_ip.map(|cip| cip.to_string());
        let expires_at = OffsetDateTime::now_utc() + Duration::from_secs(SESSION_TTL);

        tracing::debug!("accessing OAuth access token secret");
        let access_token_secret = self.access_token.secret();

        let session_id_str = sqlx::query_scalar!(
            r#"INSERT INTO sessions
                (user_id, provider, access_token, access_expires_at, refresh_token, client_ip, user_agent, expires_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                RETURNING id;"#,
            self.user_id,
            self.provider,
            access_token_secret,
            self.access_expires_at,
            self.refresh_token,
            client_ip_str,
            self.user_agent,
            expires_at,
        )
        .fetch_one(database.deref())
        .await
        .map_err(SessionError::SaveFailed)?;

        Ok(SessionId::from(session_id_str))
    }

    pub fn user_agent(&mut self, user_agent: String) -> &mut Self {
        self.user_agent = Some(user_agent);
        self
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
            .execute(database.deref())
            .await?;

        Ok(())
    }

    pub async fn locate(database: &Database, id: SessionId) -> Result<Option<Self>, sqlx::Error> {
        let query_result = sqlx::query_as!(Self, "SELECT * FROM sessions WHERE id = $1;", id,)
            .fetch_one(database.deref())
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
    #[error("saving the session to the database failed: {0}")]
    SaveFailed(sqlx::Error),
}
