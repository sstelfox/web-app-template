use std::fmt::{self, Display, Formatter};
use std::ops::Deref;

use uuid::Uuid;

use crate::database::custom_types::Did;
use crate::database::Database;

#[derive(Clone, Copy, Debug, sqlx::Type)]
#[sqlx(transparent)]
pub struct UserId(Did);

impl UserId {
    pub async fn from_email(database: &Database, email: &str) -> Result<Option<Self>, UserIdError> {
        sqlx::query_scalar!("SELECT id as 'id: UserId' FROM users WHERE email = LOWER($1);", email,)
            .fetch_optional(database.deref())
            .await
            .map_err(UserIdError::LookupFailed)
    }
}

impl Display for UserId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
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
