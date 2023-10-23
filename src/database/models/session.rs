//use std::net::IpAddr;
use std::ops::Deref;
use std::time::Duration;

use time::OffsetDateTime;

use crate::auth::SESSION_TTL;
use crate::database::custom_types::{OAuthProviderAccountId, SessionId, UserId};
use crate::database::Database;

#[derive(Debug)]
pub struct CreateSession {
    user_id: UserId,
    oauth_provider_account_id: OAuthProviderAccountId,

    client_ip: Option<String>,
    user_agent: Option<String>,

    expires_at: OffsetDateTime,
}

impl CreateSession {
    pub fn expires_at(&self) -> OffsetDateTime {
        self.expires_at.clone()
    }

    pub fn limit_duration_to(&mut self, duration: Duration) -> &mut Self {
        let upper_bound = OffsetDateTime::now_utc() + duration;

        if upper_bound < self.expires_at {
            self.expires_at = upper_bound;
        }

        self
    }

    pub async fn create(self, database: &Database) -> Result<SessionId, SessionError> {
        sqlx::query_scalar!(
            r#"INSERT INTO sessions
                (user_id, oauth_provider_account_id, client_ip, user_agent, expires_at)
                VALUES ($1, $2, $3, $4, $5)
                RETURNING id as 'id: SessionId';"#,
            self.user_id,
            self.oauth_provider_account_id,
            self.client_ip,
            self.user_agent,
            self.expires_at,
        )
        .fetch_one(database.deref())
        .await
        .map_err(SessionError::SaveFailed)
    }

    pub fn new(user_id: UserId, oauth_provider_account_id: OAuthProviderAccountId) -> Self {
        let expires_at = OffsetDateTime::now_utc() + Duration::from_secs(SESSION_TTL);

        Self {
            user_id,
            oauth_provider_account_id,

            client_ip: None,
            user_agent: None,

            expires_at,
        }
    }

    //pub fn set_client_ip(&mut self, client_ip: IpAddr) -> &mut Self {
    //    self.client_ip = Some(client_ip);
    //    self
    //}

    pub fn set_user_agent(&mut self, user_agent: String) -> &mut Self {
        self.user_agent = Some(user_agent);
        self
    }
}

#[derive(sqlx::FromRow)]
pub struct Session {
    id: SessionId,

    user_id: UserId,
    oauth_provider_account_id: OAuthProviderAccountId,

    client_ip: Option<String>,
    user_agent: Option<String>,

    created_at: OffsetDateTime,
    expires_at: OffsetDateTime,
}

impl Session {
    pub fn created_at(&self) -> OffsetDateTime {
        self.created_at
    }

    pub async fn delete(database: &Database, id: SessionId) -> Result<(), sqlx::Error> {
        let id_str = id.to_string();

        sqlx::query!("DELETE FROM sessions WHERE id = $1;", id_str)
            .execute(database.deref())
            .await?;

        Ok(())
    }

    pub fn expires_at(&self) -> OffsetDateTime {
        self.expires_at
    }

    pub fn id(&self) -> SessionId {
        self.id
    }

    pub async fn locate(database: &Database, id: SessionId) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            Self,
            r#"SELECT
                   id as 'id: SessionId',
                   user_id as 'user_id: UserId',
                   oauth_provider_account_id as 'oauth_provider_account_id: OAuthProviderAccountId',
                   client_ip,
                   user_agent,
                   created_at,
                   expires_at
                 FROM sessions
                 WHERE id = $1;"#,
            id
        )
        .fetch_optional(database.deref())
        .await
    }

    pub fn oauth_provider_account_id(&self) -> OAuthProviderAccountId {
        self.oauth_provider_account_id
    }

    pub fn user_id(&self) -> UserId {
        self.user_id
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("saving the session to the database failed: {0}")]
    SaveFailed(sqlx::Error),
}
