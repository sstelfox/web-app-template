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

#[derive(Clone)]
pub struct Secrets {
    provider_credentials: Arc<BTreeMap<Arc<str>, ProviderCredential>>,
    service_signing_key: ServiceSigningKey,
}

impl Secrets {
    pub fn new(
        credentials: BTreeMap<Arc<str>, ProviderCredential>,
        service_signing_key: ServiceSigningKey,
    ) -> Self {
        Self {
            provider_credentials: Arc::new(credentials),
            service_signing_key,
        }
    }

    pub fn provider_credential(&self, config_id: &str) -> Option<&ProviderCredential> {
        self.provider_credentials.get(config_id)
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
