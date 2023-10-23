use std::fmt::{self, Display, Formatter};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::database::custom_types::Did;

#[derive(Clone, Copy, Debug, Deserialize, Hash, Eq, Ord, PartialEq, PartialOrd, Serialize, sqlx::Type)]
#[sqlx(transparent)]
pub struct JobId(Did);

impl Display for JobId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for JobId {
    fn from(value: Uuid) -> Self {
        Self(Did::from(value))
    }
}
