use std::fmt::{self, Display, Formatter};
use std::ops::Deref;

use uuid::Uuid;

use crate::database::Database;
use crate::database::custom_types::{Did, LoginProvider, ProviderId};

#[derive(Clone, Copy, Debug, sqlx::Type)]
#[sqlx(transparent)]
pub struct OAuthProviderAccountId(Did);

impl OAuthProviderAccountId {
    pub async fn from_provider_account_id(
        database: &Database,
        provider: LoginProvider,
        provider_account_id: ProviderId,
    ) -> Result<Option<Self>, OAuthProviderAccountIdError> {
        sqlx::query_scalar!(
            r#"SELECT id as 'id: OAuthProviderAccountId'
                   FROM oauth_provider_accounts
                   WHERE provider = $1 AND provider_id = $2;"#,
            provider,
            provider_account_id,
        )
        .fetch_optional(database.deref())
        .await
        .map_err(OAuthProviderAccountIdError::LookupFailed)
    }
}

impl Display for OAuthProviderAccountId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl From<Uuid> for OAuthProviderAccountId {
    fn from(val: Uuid) -> Self {
        Self(Did::from(val))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum OAuthProviderAccountIdError {
    #[error("failed to lookup oauth provider account id: {0}")]
    LookupFailed(sqlx::Error),
}
