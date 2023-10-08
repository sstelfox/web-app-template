use std::fmt::{self, Display, Formatter};
use std::ops::Deref;

use uuid::Uuid;

use crate::database::Database;
use crate::database::custom_types::Did;

#[derive(Clone, Copy, Debug, sqlx::Type)]
#[sqlx(transparent)]
pub struct UserId(Did);

impl UserId {
    pub async fn from_email(database: &Database, email: &str) -> Result<Option<Self>, UserIdError> {
        let maybe_id: Option<String> = sqlx::query_scalar!(
            "SELECT id FROM users WHERE email = LOWER($1);",
            email,
        )
        .fetch_optional(database)
        .await
        .map_err(UserIdError::LookupFailed)?;

        match maybe_id {
            Some(sid) => Ok(Some(UserId(Did::try_from(sid).map_err(UserIdError::CorruptId)?))),
            None => Ok(None),
        }
    }
}

impl Deref for UserId {
    type Target = Uuid;

    fn deref(&self) -> &Self::Target {
        &self.0.deref()
    }
}

impl Display for UserId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl From<String> for UserId {
    fn from(val: String) -> Self {
        Self(Did::try_from(val).expect("valid user ID"))
    }
}

impl From<Uuid> for UserId {
    fn from(val: Uuid) -> Self {
        Self(Did::from(val))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum UserIdError {
    #[error("user ID present in a database is corrupt: {0}")]
    CorruptId(uuid::Error),

    #[error("failed to lookup user ID: {0}")]
    LookupFailed(sqlx::Error),
}
