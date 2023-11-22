use time::OffsetDateTime;

use crate::database::custom_types::{ApiKeyId, UserId};

#[derive(sqlx::FromRow)]
pub struct ApiKey {
    id: ApiKeyId,
    user_id: UserId,

    name: Option<String>,
    fingerprint: Vec<u8>,
    public_key: Vec<u8>,

    created_at: OffsetDateTime,
}
