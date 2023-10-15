use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, sqlx::Type)]
#[sqlx(transparent)]
pub struct ProviderId(String);
