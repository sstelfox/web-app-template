use time::OffsetDateTime;

use crate::database::custom_types::{ApiKeyId, Fingerprint, UserId};

#[derive(sqlx::FromRow)]
pub struct ApiKey {
    id: ApiKeyId,
    user_id: UserId,

    name: Option<String>,
    fingerprint: Vec<u8>,
    public_key: Vec<u8>,

    created_at: OffsetDateTime,
}

impl ApiKey {
    pub fn from_fingerprint(fingerprint: &Fingerprint) -> Result<ApiKey, &str> {
        todo!()
    }
}
