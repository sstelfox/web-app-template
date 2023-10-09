use axum::response::{IntoResponse, Response};
use axum::Json;
use http::StatusCode;
use oauth2::basic::{BasicClient, BasicTokenType};
use oauth2::{
    AuthorizationCode, CsrfToken, PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope,
};
use oauth2::{EmptyExtraTokenFields, StandardTokenResponse};
use url::Url;

use crate::app::Secrets;
use crate::auth::CALLBACK_PATH_TEMPLATE;
use crate::database::custom_types::LoginProvider;

pub struct OAuthClient {
    client: BasicClient,
    login_provider: LoginProvider,
}

impl OAuthClient {
    pub fn configure(
        login_provider: LoginProvider,
        mut redirect_url: Url,
        secrets: &Secrets,
    ) -> Result<Self, OAuthClientError> {
        let provider_credentials = secrets.provider_credential(login_provider).ok_or(
            OAuthClientError::CredentialsMissing(login_provider.as_str()),
        )?;

        let provider_config = login_provider.config();

        let auth_url = provider_config.auth_url();
        let token_url = provider_config.token_url();

        redirect_url.set_path(&CALLBACK_PATH_TEMPLATE.replace("{}", login_provider.as_str()));
        let redirect_url = RedirectUrl::from_url(redirect_url);

        let mut client = BasicClient::new(
            provider_credentials.id(),
            Some(provider_credentials.secret()),
            auth_url,
            token_url,
        )
        .set_redirect_uri(redirect_url);

        if let Some(ru) = provider_config.revocation_url() {
            client = client.set_revocation_uri(ru);
        }

        Ok(Self {
            client,
            login_provider,
        })
    }

    pub async fn generate_challenge(&self) -> Result<OAuthChallenge, OAuthClientError> {
        let provider_config = self.login_provider.config();

        let (pkce_code_challenge, pkce_code_verifier) = PkceCodeChallenge::new_random_sha256();
        let mut auth_request = self.client.authorize_url(CsrfToken::new_random);

        for scope in provider_config.scopes() {
            auth_request = auth_request.add_scope(Scope::new(scope.to_string()));
        }

        let (authorize_url, csrf_token) =
            auth_request.set_pkce_challenge(pkce_code_challenge).url();

        Ok(OAuthChallenge {
            authorize_url,
            csrf_token,
            pkce_code_verifier,
        })
    }

    pub fn validate_exchange(
        &self,
        authorization_code: AuthorizationCode,
        pkce_code_verifier: PkceCodeVerifier,
    ) -> Result<StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType>, OAuthClientError>
    {
        self.client
            .exchange_code(authorization_code)
            .set_pkce_verifier(pkce_code_verifier)
            .request(oauth2::reqwest::http_client)
            .map_err(|err| OAuthClientError::ExchangeCodeFailure(err.to_string()))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum OAuthClientError {
    #[error("unable to location credentials for '{0}' login provider")]
    CredentialsMissing(&'static str),

    #[error("failed to verify exchange code: {0}")]
    ExchangeCodeFailure(String),
}

impl IntoResponse for OAuthClientError {
    fn into_response(self) -> Response {
        {
            tracing::error!("{self}");
            let err_msg = serde_json::json!({"msg": "backend service experienced an issue servicing the request"});
            (StatusCode::INTERNAL_SERVER_ERROR, Json(err_msg)).into_response()
        }
    }
}

pub struct OAuthChallenge {
    pub authorize_url: Url,
    pub csrf_token: CsrfToken,
    pub pkce_code_verifier: PkceCodeVerifier,
}
