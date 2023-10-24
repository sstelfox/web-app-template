use std::fmt::{self, Display, Formatter};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::database::custom_types::Did;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, sqlx::Type)]
#[sqlx(transparent)]
pub struct BackgroundRunId(Did);

impl Display for BackgroundRunId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl From<Uuid> for BackgroundRunId {
    fn from(val: Uuid) -> Self {
        Self(Did::from(val))
    }
}
