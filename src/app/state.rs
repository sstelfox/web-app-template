use std::sync::Arc;

use jwt_simple::algorithms::{ES384KeyPair, ECDSAP384KeyPairLike};
use sha2::Digest;

use crate::app::{Config, Error};
use crate::database::{config_database, Database};

#[derive(Clone)]
pub struct State {
    database: Database,
    jwt_key: Arc<ES384KeyPair>,
}

impl State {
    // not implemented as a From trait so it can be async
    pub async fn from_config(config: &Config) -> Result<Self, Error> {
        let database = config_database(&config).await?;

        let mut jwt_key_raw = match config.jwt_key_path() {
            Some(path) => {
                let key_bytes = std::fs::read(path).map_err(Error::unreadable_key)?;
                let pem = String::from_utf8_lossy(&key_bytes);

                ES384KeyPair::from_pem(&pem).map_err(Error::invalid_key)?
            }
            None => ES384KeyPair::generate(),
        };

        let fingerprint = fingerprint_jwt_key(&jwt_key_raw);
        jwt_key_raw = jwt_key_raw.with_key_id(&fingerprint);
        let jwt_key = Arc::new(jwt_key_raw);

        Ok(Self { database, jwt_key })
    }
}

impl axum::extract::FromRef<State> for Database {
    fn from_ref(state: &State) -> Self {
        state.database.clone()
    }
}

impl axum::extract::FromRef<State> for Arc<ES384KeyPair> {
    fn from_ref(state: &State) -> Self {
        state.jwt_key.clone()
    }
}

fn fingerprint_jwt_key(jwt_keys: &ES384KeyPair) -> String {
    let public_key = jwt_keys.key_pair().public_key();
    let compressed_point = public_key.as_ref().to_encoded_point(true);

    let mut hasher = sha2::Sha256::new();
    hasher.update(compressed_point);
    let hashed_bytes = hasher.finalize();

    hashed_bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
