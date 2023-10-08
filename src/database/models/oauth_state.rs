use oauth2::PkceCodeVerifier;

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

#[derive(sqlx::FromRow)]
pub struct VerifyOAuthState {
    pkce_verifier_secret: String,
    post_login_redirect_url: Option<String>,
}

impl VerifyOAuthState {
    pub async fn delete(
        database: &Database,
        provider: LoginProvider,
        csrf_secret: String,
    ) -> Result<Self, sqlx::Error> {
        todo!()
    }

    pub async fn locate(
        database: &Database,
        provider: LoginProvider,
        csrf_secret: String,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as!(
            Self,
            r#"SELECT pkce_verifier_secret,post_login_redirect_url
                FROM oauth_state
                WHERE provider = $1 AND csrf_secret = $2;"#,
            provider,
            csrf_secret,
        )
        .fetch_one(database)
        .await
    }

    pub async fn locate_and_delete(
        database: &Database,
        provider: LoginProvider,
        csrf_secret: String,
    ) -> Result<Self, sqlx::Error> {
        let found_state = Self::locate(database, provider, csrf_secret.clone()).await?;

        if let Err(err) = Self::delete(database, provider, csrf_secret).await {
            tracing::warn!("failed to clean up oauth state: {err}");
        }

        Ok(found_state)
    }

    pub fn pkce_code_verifier(&self) ->  PkceCodeVerifier {
        PkceCodeVerifier::new(self.pkce_verifier_secret.clone())
    }

    pub fn post_login_redirect_url(&self) -> Option<String> {
        self.post_login_redirect_url.clone()
    }
}
