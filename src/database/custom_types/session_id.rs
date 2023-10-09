use std::fmt::{self, Display, Formatter};
use std::ops::Deref;

use uuid::Uuid;

use crate::database::custom_types::Did;

#[derive(Clone, Copy, Debug, sqlx::Type)]
#[sqlx(transparent)]
pub struct SessionId(Did);

impl Deref for SessionId {
    type Target = Uuid;

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

impl Display for SessionId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for SessionId {
    fn from(val: String) -> Self {
        Self(Did::try_from(val).expect("session ID to be valid"))
    }
}

impl From<Uuid> for SessionId {
    fn from(val: Uuid) -> Self {
        Self(Did::from(val))
    }
}
