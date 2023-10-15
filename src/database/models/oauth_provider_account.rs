use std::ops::Deref;

use time::OffsetDateTime;
use url::Url;

use crate::database::custom_types::{LoginProvider, OAuthProviderAccountId, UserId};
use crate::database::Database;

#[derive(sqlx::FromRow)]
pub struct OAuthProviderAccount {
    id: OAuthProviderAccountId,

    user_id: UserId,

    provider: LoginProvider,
    provider_id: String,
    provider_email: String,

    associated_at: OffsetDateTime,
}

#[derive(Debug, thiserror::Error)]
pub enum OAuthProviderAccountError {
    #[error("failed to save oauth provider account: {0}")]
    SaveFailed(sqlx::Error),
}
