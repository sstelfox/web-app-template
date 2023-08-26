use jwt_simple::algorithms::{ES384KeyPair, ECDSAP384KeyPairLike};
use sha2::Digest;

use crate::app::{Config, Error, SessionCreator, SessionVerifier};
use crate::database::{config_database, Db};

#[derive(Clone)]
pub struct State {
    database: Db,
    session_key: SessionCreator,
    session_verifier: SessionVerifier,
}

impl State {
    // not implemented as a From trait so it can be async
    pub async fn from_config(config: &Config) -> Result<Self, Error> {
        let database = config_database(&config).await?;

        // load our key
        let path = config.session_key_path();
        let key_bytes = std::fs::read(path).map_err(Error::unreadable_key)?;
        let pem = String::from_utf8_lossy(&key_bytes);
        let mut session_key_raw = ES384KeyPair::from_pem(&pem).map_err(Error::invalid_key)?;

        // mark it with a calculated fingerprint
        let fingerprint = fingerprint_key(&session_key_raw);
        session_key_raw = session_key_raw.with_key_id(&fingerprint);

        // wrap our key and verifier
        let session_key = SessionCreator::new(session_key_raw);
        let session_verifier = session_key.verifier();

        Ok(Self { database, session_key, session_verifier })
    }
}

impl axum::extract::FromRef<State> for Db {
    fn from_ref(state: &State) -> Self {
        state.database.clone()
    }
}

impl axum::extract::FromRef<State> for SessionCreator {
    fn from_ref(state: &State) -> Self {
        state.session_key.clone()
    }
}

impl axum::extract::FromRef<State> for SessionVerifier {
    fn from_ref(state: &State) -> Self {
        state.session_verifier.clone()
    }
}

fn fingerprint_key(keys: &ES384KeyPair) -> String {
    let public_key = keys.key_pair().public_key();
    let compressed_point = public_key.as_ref().to_encoded_point(true);

    let mut hasher = sha2::Sha256::new();
    hasher.update(compressed_point);
    let hashed_bytes = hasher.finalize();

    hashed_bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
