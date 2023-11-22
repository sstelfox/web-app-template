use std::fmt::{self, Display, Formatter};
use std::ops::Deref;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::database::custom_types::Did;
use crate::database::{Database, DatabaseConnection};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, sqlx::Type)]
#[sqlx(transparent)]
pub struct UserId(Did);

impl UserId {
    pub async fn from_email(
        database: &mut DatabaseConnection,
        email: &str,
    ) -> Result<Option<Self>, UserIdError> {
        sqlx::query_scalar!(
            "SELECT id as 'id: UserId' FROM users WHERE email = LOWER($1);",
            email,
        )
        .fetch_optional(&mut *database)
        .await
        .map_err(UserIdError::LookupFailed)
    }
}

impl Display for UserId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl From<Uuid> for UserId {
    fn from(val: Uuid) -> Self {
        Self(Did::from(val))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum UserIdError {
    #[error("failed to lookup user ID: {0}")]
    LookupFailed(sqlx::Error),
}
