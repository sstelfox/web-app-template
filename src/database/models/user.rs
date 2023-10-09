use std::ops::Deref;

use time::OffsetDateTime;
use url::Url;

use crate::database::custom_types::UserId;
use crate::database::Database;

pub struct CreateUser {
    email: String,
    display_name: String,
    locale: Option<String>,
    profile_image: Option<Url>,
}

impl CreateUser {
    pub fn locale(&mut self, locale: String) -> &mut Self {
        self.locale = Some(locale);
        self
    }

    pub fn new(email: String, display_name: String) -> Self {
        Self {
            email,
            display_name,
            locale: None,
            profile_image: None,
        }
    }

    pub fn profile_image(&mut self, profile_image: Url) -> &mut Self {
        self.profile_image = Some(profile_image);
        self
    }

    pub async fn save(self, database: &Database) -> Result<UserId, UserError> {
        let profile_image_str = self.profile_image.map(|i| i.to_string());

        let user_id_str = sqlx::query_scalar!(
            r#"INSERT INTO users (email, display_name, locale, profile_image)
                VALUES (LOWER($1), $2, $3, $4)
                RETURNING id;"#,
            self.email,
            self.display_name,
            self.locale,
            profile_image_str,
        )
        .fetch_one(database.deref())
        .await
        .map_err(UserError::SaveFailed)?;

        Ok(UserId::from(user_id_str))
    }
}

#[derive(sqlx::FromRow)]
pub struct User {
    id: UserId,

    email: String,
    display_name: String,

    locale: Option<String>,
    profile_image: Option<String>,

    created_at: OffsetDateTime,
}

#[derive(Debug, thiserror::Error)]
pub enum UserError {
    #[error("failed to save new user: {0}")]
    SaveFailed(sqlx::Error),
}
