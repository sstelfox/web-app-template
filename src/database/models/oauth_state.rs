use crate::database::Database;
use crate::database::custom_types::{Did, LoginProvider};

pub struct NewOAuthState {
    pub provider: LoginProvider,
    pub csrf_secret: String,
    pub pkce_verifier_secret: String,
    pub post_login_redirect_url: Option<String>,
}

impl NewOAuthState {
    pub async fn save(self, database: &Database) -> Result<OAuthStateId, sqlx::Error> {
        todo!()
    }
}

#[derive(sqlx::Type)]
pub struct OAuthStateId(Did);
