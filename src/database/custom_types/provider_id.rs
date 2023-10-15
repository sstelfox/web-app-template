use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize, sqlx::Type)]
#[sqlx(transparent)]
pub struct ProviderId(String);
