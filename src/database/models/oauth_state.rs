use crate::database::Database;
use crate::database::custom_types::LoginProvider;

pub struct NewOAuthState {
    provider: LoginProvider,
    csrf_secret: String,
    pkce_verifier_secret: String,
    post_login_redirect_url: Option<String>,
}

impl NewOAuthState {
    pub fn delete(
        database: &Database,
        provider: LoginProvider,
        csrf_secret: String,
    ) -> Result<(), sqlx::Error> {
        todo!()
    }

    pub fn locate(
        database: &Database,
        provider: LoginProvider,
        csrf_secret: String,
    ) -> Result<Option<Self>, sqlx::Error> {
        todo!()
    }

    pub fn new(
        provider: LoginProvider,
        csrf_secret: String,
        pkce_verifier_secret: String,
        post_login_redirect_url: Option<String>,
    ) -> Self {
        Self {
            provider,
            csrf_secret,
            pkce_verifier_secret,
            post_login_redirect_url,
        }
    }

    pub async fn save(self, database: &Database) -> Result<(), sqlx::Error> {
        sqlx::query_scalar!(
            r#"INSERT INTO oauth_state (provider, csrf_secret, pkce_verifier_secret, post_login_redirect_url)
                   VALUES ($1, $2, $3, $4);"#,
            self.provider,
            self.csrf_secret,
            self.pkce_verifier_secret,
            self.post_login_redirect_url,
        )
        .execute(database)
        .await?;

        Ok(())
    }
}
