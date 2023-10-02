use axum::extract::FromRef;
use jwt_simple::algorithms::{ECDSAP384KeyPairLike, ES384KeyPair};
use sha2::Digest;
use std::io::Write;

use crate::app::{Config, Error, SessionCreationKey, SessionVerificationKey};
use crate::database::{self, Database};

#[derive(Clone)]
pub struct State {
    database: Database,
    session_key: SessionCreationKey,
    session_verifier: SessionVerificationKey,
}

impl State {
    // not implemented as a From trait so it can be async
    pub async fn from_config(config: &Config) -> Result<Self, Error> {
        let database = database::connect(&config.db_url()).await?;
        let path = config.session_key_path();

        let mut session_key_raw = if path.exists() {
            // load our key
            tracing::info!(key_path = ?path, "loading session key");
            let key_bytes = std::fs::read(path).map_err(Error::UnreadableSessionKey)?;
            let pem = String::from_utf8_lossy(&key_bytes);
            ES384KeyPair::from_pem(&pem).map_err(Error::InvalidSessionKey)?
        } else {
            // generate a fresh key and write it out
            tracing::warn!(key_path = ?path, "generating new session key");

            let key = ES384KeyPair::generate();
            let pem_key = key.to_pem().expect("fresh keys to export");

            // don't allow overwriting a key if it already exists
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path.clone())
                .map_err(|err| Error::UnwritableSessionKey(err))?;

            file.write_all(pem_key.as_bytes())
                .map_err(|err| Error::UnwritableSessionKey(err))?;

            key
        };

        // mark it with a calculated fingerprint
        let fingerprint = fingerprint_key(&session_key_raw);
        session_key_raw = session_key_raw.with_key_id(&fingerprint);

        // wrap our key and verifier
        let session_key = SessionCreationKey::new(session_key_raw);
        let session_verifier = session_key.verifier();

        Ok(Self {
            database,
            session_key,
            session_verifier,
        })
    }
}

impl FromRef<State> for Database {
    fn from_ref(state: &State) -> Self {
        state.database.clone()
    }
}

impl FromRef<State> for SessionCreationKey {
    fn from_ref(state: &State) -> Self {
        state.session_key.clone()
    }
}

impl FromRef<State> for SessionVerificationKey {
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
