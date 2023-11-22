use std::fmt::{self, Display, Formatter};

use serde::{Deserialize, Serialize};

use crate::database::custom_types::Did;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, sqlx::Type)]
#[sqlx(transparent)]
pub struct ApiKeyId(Did);

impl Display for ApiKeyId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
