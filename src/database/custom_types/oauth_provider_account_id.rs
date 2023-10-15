use std::fmt::{self, Display, Formatter};
use std::ops::Deref;

use uuid::Uuid;

use crate::database::Database;
use crate::database::custom_types::Did;

#[derive(Clone, Copy, Debug, sqlx::Type)]
#[sqlx(transparent)]
pub struct OAuthProviderAccountId(Did);

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
