use std::collections::BTreeMap;
use std::fmt::Write;
use std::path::PathBuf;

use axum::extract::FromRef;
use jwt_simple::prelude::*;
use object_store::local::LocalFileSystem;
use sha2::Digest;

use crate::app::{
    Config, ProviderCredential, Secrets, ServiceSigningKey, ServiceVerificationKey, UploadStore,
};
use crate::background_jobs::{BasicTaskContext, BasicTaskStore, EventTaskContext, EventTaskStore};
use crate::database::custom_types::LoginProvider;
use crate::database::{Database, DatabaseSetupError};
use crate::event_bus::EventBus;

#[derive(Clone)]
pub struct AppState {
    database: Database,
    event_bus: EventBus,
    secrets: Secrets,

    service_verifier: ServiceVerificationKey,
    upload_directory: PathBuf,
}

impl AppState {
    pub fn database(&self) -> Database {
        self.database.clone()
    }

    pub fn event_bus(&self) -> EventBus {
        self.event_bus.clone()
    }

    pub async fn from_config(config: &Config) -> Result<Self, AppStateSetupError> {
        let database = Database::connect(&config.database_url()).await?;
        let event_bus = EventBus::new();

        let service_key = load_or_create_service_key(&config.service_key_path())?;
        let service_verifier = service_key.verifier();

        let mut credentials = BTreeMap::new();
        credentials.insert(
            LoginProvider::Google,
            ProviderCredential::new(config.google_client_id(), config.google_client_secret()),
        );
        let secrets = Secrets::new(credentials, service_key);

        Ok(Self {
            database,
            event_bus,
            secrets,
            service_verifier,
            upload_directory: config.upload_directory(),
        })
    }

    pub fn secrets(&self) -> Secrets {
        self.secrets.clone()
    }

    pub fn service_verifier(&self) -> ServiceVerificationKey {
        self.service_verifier.clone()
    }

    pub fn basic_task_store(&self) -> BasicTaskStore {
        let context = BasicTaskContext::new(self.database());
        BasicTaskStore::new(context)
    }

    pub fn event_task_store(&self) -> EventTaskStore {
        let context = EventTaskContext::new(self.database(), self.event_bus());
        EventTaskStore::new(context)
    }

    pub fn upload_store(&self) -> Result<UploadStore, AppStateError> {
        let local_fs = LocalFileSystem::new_with_prefix(&self.upload_directory)
            .map_err(AppStateError::UploadStoreUnavailable)?;

        Ok(UploadStore::new(local_fs))
    }
}

impl FromRef<AppState> for Database {
    fn from_ref(state: &AppState) -> Self {
        state.database()
    }
}

impl FromRef<AppState> for Secrets {
    fn from_ref(state: &AppState) -> Self {
        state.secrets()
    }
}

impl FromRef<AppState> for ServiceVerificationKey {
    fn from_ref(state: &AppState) -> Self {
        state.service_verifier()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AppStateError {
    #[error("unable to get a handle on the upload store: {0}")]
    UploadStoreUnavailable(object_store::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum AppStateSetupError {
    #[error("private service key could not be loaded: {0}")]
    InvalidServiceKey(jwt_simple::Error),

    #[error("failed to setup the database: {0}")]
    DatabaseSetupError(#[from] DatabaseSetupError),

    #[error("failed to write fingerprint: {0}")]
    FingerprintWriteFailed(std::io::Error),

    #[error("failed to write public key: {0}")]
    PublicKeyWriteFailed(std::io::Error),

    #[error("unable to write generated service key: {0}")]
    ServiceKeyWriteFailed(std::io::Error),

    #[error("failed to read private service key: {0}")]
    UnreadableServiceKey(std::io::Error),
}

fn fingerprint_key(keys: &ES384KeyPair) -> String {
    let public_key = keys.key_pair().public_key();
    let compressed_point = public_key.as_ref().to_encoded_point(true);

    let mut hasher = sha2::Sha256::new();
    hasher.update(compressed_point);
    let hashed_bytes = hasher.finalize();

    hashed_bytes.iter().fold(String::new(), |mut output, b| {
        let _ = write!(output, "{b:02x}");
        output
    })
}

fn load_or_create_service_key(
    private_path: &PathBuf,
) -> Result<ServiceSigningKey, AppStateSetupError> {
    let mut session_key_raw = if private_path.exists() {
        let key_bytes =
            std::fs::read(private_path).map_err(AppStateSetupError::UnreadableServiceKey)?;
        let private_pem = String::from_utf8_lossy(&key_bytes);

        ES384KeyPair::from_pem(&private_pem).map_err(AppStateSetupError::InvalidServiceKey)?
    } else {
        let new_key = ES384KeyPair::generate();
        let private_pem = new_key.to_pem().expect("fresh keys to export");

        std::fs::write(private_path, private_pem)
            .map_err(AppStateSetupError::ServiceKeyWriteFailed)?;

        let public_spki = new_key
            .public_key()
            .to_pem()
            .expect("fresh key to have public component");
        let mut public_path = private_path.clone();
        public_path.set_extension("public");
        std::fs::write(public_path, public_spki)
            .map_err(AppStateSetupError::PublicKeyWriteFailed)?;

        new_key
    };

    let fingerprint = fingerprint_key(&session_key_raw);
    session_key_raw = session_key_raw.with_key_id(&fingerprint);

    let mut fingerprint_path = private_path.clone();
    fingerprint_path.set_extension("fingerprint");
    if !fingerprint_path.exists() {
        std::fs::write(fingerprint_path, fingerprint)
            .map_err(AppStateSetupError::FingerprintWriteFailed)?;
    }

    Ok(ServiceSigningKey::new(session_key_raw))
}
