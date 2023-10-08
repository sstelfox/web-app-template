use time::OffsetDateTime;

use crate::database::custom_types::UserId;

#[derive(sqlx::FromRow)]
pub struct User {
    id: UserId,

    email: String,
    display_name: String,

    locale: Option<String>,
    profile_image: Option<String>,

    created_at: OffsetDateTime,
}
