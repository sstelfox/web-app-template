use std::fmt::{self, Debug, Formatter};
use std::ops::Deref;

use uuid::Uuid;

#[derive(Clone, Copy, sqlx::Type)]
#[sqlx(transparent)]
pub struct Did(Uuid);

impl Debug for Did {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Deref for Did {
    type Target = Uuid;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Uuid> for Did {
    fn from(val: Uuid) -> Self {
        Self(val)
    }
}

impl TryFrom<String> for Did {
    type Error = uuid::Error;

    fn try_from(val: String) -> Result<Self, Self::Error> {
        Uuid::parse_str(&val).map(Self)
    }
}
