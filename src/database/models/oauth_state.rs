use std::ops::Deref;

use oauth2::{CsrfToken, PkceCodeVerifier};

use crate::database::custom_types::LoginProvider;
use crate::database::Database;

pub struct CreateOAuthState {
    provider: LoginProvider,
    csrf_token: CsrfToken,
    pkce_code_verifier: PkceCodeVerifier,
    post_login_redirect_url: Option<String>,
}

impl CreateOAuthState {
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
        tracing::debug!("accessing OAuth PKCE code verification secret");
        self.pkce_code_verifier.secret().to_string()
    }

    pub async fn save(self, database: &Database) -> Result<(), OAuthStateError> {
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
        .execute(database.deref())
        .await
        .map_err(OAuthStateError::Creating)?;

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
    ) -> Result<(), OAuthStateError> {
        tracing::debug!("accessing OAuth CSRF token secret to delete session");
        let csrf_token_secret = csrf_token.secret().to_string();

        sqlx::query_as!(
            Self,
            "DELETE FROM oauth_state WHERE provider = $1 AND csrf_token_secret = $2;",
            provider,
            csrf_token_secret,
        )
        .execute(database.deref())
        .await
        .map_err(OAuthStateError::Deleting)?;

        Ok(())
    }

    pub async fn locate(
        database: &Database,
        provider: LoginProvider,
        csrf_token: CsrfToken,
    ) -> Result<Option<Self>, OAuthStateError> {
        tracing::debug!("accessing OAuth CSRF token secret to locate session");
        let csrf_token_secret = csrf_token.secret().to_string();

        sqlx::query_as!(
            Self,
            r#"SELECT pkce_code_verifier_secret, post_login_redirect_url
                   FROM oauth_state
                   WHERE provider = $1 AND csrf_token_secret = $2 AND created_at >= DATETIME('now', '-5 minute');"#,
            provider,
            csrf_token_secret,
        )
        .fetch_optional(database.deref())
        .await
        .map_err(OAuthStateError::Locating)
    }

    pub async fn locate_and_delete(
        database: &Database,
        provider: LoginProvider,
        csrf_token: CsrfToken,
    ) -> Result<Option<Self>, OAuthStateError> {
        let found_state = Self::locate(database, provider, csrf_token.clone()).await?;

        // failing to delete this row shouldn't prevent logins
        if let Err(err) = Self::delete(database, provider, csrf_token).await {
            tracing::warn!("failed to clean up oauth state: {err}");
        }

        Ok(found_state)
    }

    pub fn pkce_code_verifier(&self) -> PkceCodeVerifier {
        PkceCodeVerifier::new(self.pkce_code_verifier_secret.clone())
    }

    pub fn post_login_redirect_url(&self) -> Option<String> {
        self.post_login_redirect_url.clone()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum OAuthStateError {
    #[error("failed to create new database session: {0}")]
    Creating(sqlx::Error),

    #[error("failed to locate existing database session: {0}")]
    Locating(sqlx::Error),

    #[error("failed to delete existing database session: {0}")]
    Deleting(sqlx::Error),
}
