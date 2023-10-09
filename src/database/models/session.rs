use std::net::IpAddr;
use std::ops::Deref;
use std::time::Duration;

use oauth2::AccessToken;
use time::OffsetDateTime;

use crate::auth::SESSION_TTL;
use crate::database::custom_types::{LoginProvider, SessionId, UserId};
use crate::database::Database;

#[derive(Debug)]
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
        let user_id_str = self.user_id.to_string();
        let client_ip_str = self.client_ip.map(|cip| cip.to_string());
        let expires_at = OffsetDateTime::now_utc() + Duration::from_secs(SESSION_TTL);

        tracing::debug!("accessing OAuth access token secret to save it to the database");
        let access_token_secret = self.access_token.secret();

        tracing::warn!("{:?}", self);

        let session_id_str = sqlx::query_scalar!(
            r#"INSERT INTO sessions
                (user_id, provider, access_token_secret, access_expires_at, refresh_token, client_ip, user_agent, expires_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                RETURNING id;"#,
            user_id_str,
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
    access_token_secret: String,
    access_expires_at: Option<OffsetDateTime>,
    refresh_token: Option<String>,

    client_ip: Option<String>,
    user_agent: Option<String>,

    created_at: OffsetDateTime,
    expires_at: OffsetDateTime,
}

impl Session {
    pub fn created_at(&self) -> OffsetDateTime {
        self.created_at.clone()
    }

    pub async fn delete(database: &Database, id: SessionId) -> Result<(), sqlx::Error> {
        sqlx::query!("DELETE FROM sessions WHERE id = $1;", id)
            .execute(database.deref())
            .await?;

        Ok(())
    }

    pub fn expires_at(&self) -> OffsetDateTime {
        self.expires_at.clone()
    }

    pub fn id(&self) -> SessionId {
        self.id.clone()
    }

    pub async fn locate(database: &Database, id: SessionId) -> Result<Option<Self>, sqlx::Error> {
        let query_result = sqlx::query_as!(Self, "SELECT * FROM sessions WHERE id = $1;", id)
            .fetch_one(database.deref())
            .await;

        match query_result {
            Ok(sess) => Ok(Some(sess)),
            Err(sqlx::Error::RowNotFound) => Ok(None),
            Err(err) => Err(err),
        }
    }

    pub fn user_id(&self) -> UserId {
        self.user_id.clone()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("saving the session to the database failed: {0}")]
    SaveFailed(sqlx::Error),
}
