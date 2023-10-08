use sqlx::types::uuid::Uuid;

#[derive(sqlx::Type)]
pub struct Did(Uuid);
