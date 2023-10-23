use std::fmt::{self, Display, Formatter};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::database::custom_types::Did;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, sqlx::Type)]
#[sqlx(transparent)]
pub struct SessionId(Did);

impl SessionId {
    pub fn to_bytes_le(self) -> [u8; 16] {
        self.0.to_bytes_le()
    }
}

impl Display for SessionId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for SessionId {
    fn from(val: Uuid) -> Self {
        Self(Did::from(val))
    }
}
