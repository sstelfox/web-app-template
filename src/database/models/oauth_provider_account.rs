use std::ops::Deref;

use time::OffsetDateTime;
use url::Url;

use crate::database::custom_types::{LoginProvider, OAuthProviderAccountId, ProviderId, UserId};
use crate::database::Database;

#[derive(sqlx::FromRow)]
pub struct OAuthProviderAccount {
    id: OAuthProviderAccountId,

    user_id: UserId,

    provider: LoginProvider,
    provider_id: ProviderId,
    provider_email: String,

    associated_at: OffsetDateTime,
}

impl OAuthProviderAccount{
    pub async fn from_provider_id(
        database: &Database,
        provider: LoginProvider,
        provider_id: ProviderId,
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
                    WHERE provider = $1 AND provider_id = $2;"#,
            provider,
            provider_id,
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
