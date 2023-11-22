#![allow(dead_code)]

use std::ops::Deref;

use time::OffsetDateTime;

use crate::database::custom_types::UserId;
use crate::database::DatabaseConnection;

pub struct CreateUser<'a> {
    email: &'a str,
    display_name: &'a str,
}

impl<'a> CreateUser<'a> {
    pub fn new(email: &'a str, display_name: &'a str) -> Self {
        Self {
            email,
            display_name,
        }
    }

    pub async fn save(self, database: &mut DatabaseConnection) -> Result<UserId, UserError> {
        sqlx::query_scalar!(
            r#"INSERT INTO users (email, display_name)
                VALUES (LOWER($1), $2)
                RETURNING id as 'id: UserId';"#,
            self.email,
            self.display_name,
        )
        .fetch_one(&mut *database)
        .await
        .map_err(UserError::SaveFailed)
    }
}

#[derive(sqlx::FromRow)]
pub struct User {
    id: UserId,

    email: String,
    display_name: String,

    created_at: OffsetDateTime,
}

#[derive(Debug, thiserror::Error)]
pub enum UserError {
    #[error("failed to save new user: {0}")]
    SaveFailed(sqlx::Error),
}
