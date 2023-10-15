use std::ops::Deref;

use time::OffsetDateTime;
use url::Url;

use crate::database::custom_types::{LoginProvider, OAuthProviderAccountId, ProviderId, UserId};
use crate::database::Database;

pub struct CreateOAuthProviderAccount {
    user_id: UserId,
    provider: LoginProvider,
    provider_id: ProviderId,
    provider_email: String,
}

impl CreateOAuthProviderAccount {
    pub fn new(
        user_id: UserId,
        provider: LoginProvider,
        provider_id: ProviderId,
        provider_email: String,
    ) -> Self {
        Self {
            user_id,
            provider,
            provider_id,
            provider_email,
        }
    }

    pub async fn save(self, database: &Database) -> Result<OAuthProviderAccountId, OAuthProviderAccountError> {
        sqlx::query_scalar!(
            r#"INSERT INTO oauth_provider_accounts (user_id, provider, provider_id, provider_email)
                VALUES ($1, $2, $3, LOWER($4))
                RETURNING id as 'id: OAuthProviderAccountId';"#,
            self.user_id,
            self.provider,
            self.provider_id,
            self.provider_email,
        )
        .fetch_one(database.deref())
        .await
        .map_err(OAuthProviderAccountError::SaveFailed)
    }
}

#[derive(sqlx::FromRow)]
pub struct OAuthProviderAccount {
    id: OAuthProviderAccountId,

    user_id: UserId,

    provider: LoginProvider,
    provider_id: ProviderId,
    provider_email: String,

    associated_at: OffsetDateTime,
}

impl OAuthProviderAccount {
    pub async fn lookup_by_id(
        database: &Database,
        id: OAuthProviderAccountId,
    ) -> Result<Option<Self>, OAuthProviderAccountError> {
        sqlx::query_as!(
            Self,
            r#"SELECT
                        id as 'id: OAuthProviderAccountId',
                        user_id as 'user_id: UserId',
                        provider as 'provider: LoginProvider',
                        provider_id as 'provider_id: ProviderId',
                        provider_email,
                        associated_at
                    FROM oauth_provider_accounts
                    WHERE id = $1;"#,
            id,
        )
        .fetch_optional(database.deref())
        .await
        .map_err(OAuthProviderAccountError::LookupFailed)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum OAuthProviderAccountError {
    #[error("failed to lookup oauth provider account: {0}")]
    LookupFailed(sqlx::Error),

    #[error("failed to save oauth provider account: {0}")]
    SaveFailed(sqlx::Error),
}
