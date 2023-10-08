use oauth2::{CsrfToken, PkceCodeVerifier};

use crate::database::Database;
use crate::database::custom_types::LoginProvider;

pub struct NewOAuthState {
    provider: LoginProvider,
    csrf_token: CsrfToken,
    pkce_code_verifier: PkceCodeVerifier,
    post_login_redirect_url: Option<String>,
}

impl NewOAuthState {
    fn csrf_token_secret(&self) -> String {
        tracing::debug!("accessing OAuth CSRF token secret");
        self.csrf_token.secret().to_string()
    }

    pub fn new(
        provider: LoginProvider,
        csrf_token: CsrfToken,
        pkce_code_verifier: PkceCodeVerifier,
        post_login_redirect_url: Option<String>,
    ) -> Self {
        Self {
            provider,
            csrf_token,
            pkce_code_verifier,
            post_login_redirect_url,
        }
    }

    fn pkce_code_verifier_secret(&self) -> String {
        tracing::debug!("accessing OAuth PKCE Code Verification secret");
        self.pkce_code_verifier.secret().to_string()
    }

    pub async fn save(self, database: &Database) -> Result<(), sqlx::Error> {
        let csrf_token_secret = self.csrf_token_secret();
        let pkce_code_verifier_secret = self.pkce_code_verifier_secret();

        sqlx::query_scalar!(
            r#"INSERT INTO oauth_state (provider, csrf_token_secret, pkce_code_verifier_secret, post_login_redirect_url)
                   VALUES ($1, $2, $3, $4);"#,
            self.provider,
            csrf_token_secret,
            pkce_code_verifier_secret,
            self.post_login_redirect_url,
        )
        .execute(database)
        .await?;

        Ok(())
    }
}

#[derive(sqlx::FromRow)]
pub struct VerifyOAuthState {
    pkce_code_verifier_secret: String,
    post_login_redirect_url: Option<String>,
}

impl VerifyOAuthState {
    pub async fn delete(
        database: &Database,
        provider: LoginProvider,
        csrf_token: CsrfToken,
    ) -> Result<Self, sqlx::Error> {
        todo!()
    }

    pub async fn locate(
        database: &Database,
        provider: LoginProvider,
        csrf_token: CsrfToken,
    ) -> Result<Self, sqlx::Error> {
        tracing::debug!("accessing OAuth CSRF token secret");
        let csrf_token_secret = csrf_token.secret();

        sqlx::query_as!(
            Self,
            r#"SELECT pkce_code_verifier_secret, post_login_redirect_url
                FROM oauth_state
                WHERE provider = $1 AND csrf_token_secret = $2;"#,
            provider,
            csrf_token_secret,
        )
        .fetch_one(database)
        .await
    }

    pub async fn locate_and_delete(
        database: &Database,
        provider: LoginProvider,
        csrf_token: CsrfToken,
    ) -> Result<Self, sqlx::Error> {
        let found_state = Self::locate(database, provider, csrf_token.clone()).await?;

        if let Err(err) = Self::delete(database, provider, csrf_token).await {
            tracing::warn!("failed to clean up oauth state: {err}");
        }

        Ok(found_state)
    }

    pub fn pkce_code_verifier(&self) ->  PkceCodeVerifier {
        PkceCodeVerifier::new(self.pkce_code_verifier_secret.clone())
    }

    pub fn post_login_redirect_url(&self) -> Option<String> {
        self.post_login_redirect_url.clone()
    }
}
