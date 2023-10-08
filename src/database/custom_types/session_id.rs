use std::fmt::{self, Display, Formatter};

use uuid::Uuid;

use crate::database::custom_types::Did;

#[derive(Clone, Copy, Debug, sqlx::Type)]
#[sqlx(transparent)]
pub struct SessionId(Did);

impl Display for SessionId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
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
