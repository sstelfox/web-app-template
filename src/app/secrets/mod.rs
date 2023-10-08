use std::collections::BTreeMap;
use std::convert::Infallible;
use std::sync::Arc;

use axum::async_trait;
use axum::extract::{FromRef, FromRequestParts};
use http::request::Parts;

mod provider_credential;
mod service_signing_key;

pub use provider_credential::ProviderCredential;
pub use service_signing_key::ServiceSigningKey;

use crate::app::State;
use crate::database::custom_types::LoginProvider;

#[derive(Clone)]
pub struct Secrets {
    provider_credentials: Arc<BTreeMap<LoginProvider, ProviderCredential>>,
    service_signing_key: ServiceSigningKey,
}

impl Secrets {
    pub fn new(
        credentials: BTreeMap<LoginProvider, ProviderCredential>,
        service_signing_key: ServiceSigningKey,
    ) -> Self {
        Self {
            provider_credentials: Arc::new(credentials),
            service_signing_key,
        }
    }

    pub fn provider_credential(&self, provider: LoginProvider) -> Option<&ProviderCredential> {
        self.provider_credentials.get(&provider)
    }

    pub fn service_signing_key(&self) -> ServiceSigningKey {
        self.service_signing_key.clone()
    }
}

#[async_trait]
impl FromRequestParts<State> for Secrets {
    type Rejection = Infallible;

    async fn from_request_parts(
        _parts: &mut Parts,
        state: &State,
    ) -> Result<Self, Self::Rejection> {
        Ok(Secrets::from_ref(state))
    }
}
