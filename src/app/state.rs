use std::sync::Arc;

use jwt_simple::algorithms::ES384KeyPair;

use crate::app::{Config, Error};

#[derive(Clone)]
pub struct State {
    session_key: Arc<ES384KeyPair>,
}

impl State {
    // not implemented as a From trait so it can be async
    pub async fn from_config(config: &Config) -> Result<Self, Error> {
        let mut session_key_raw = match config.session_key_path() {
            Some(path) => {
                let key_bytes = std::fs::read(path).map_err(Error::unreadable_key)?;
                let pem = String::from_utf8_lossy(&key_bytes);
                ES384KeyPair::from_pem(&pem).map_err(Error::invalid_key)?
            }
            None => ES384KeyPair::generate(),
        };

        // todo: this probably isn't needed but elsewhere this ID is important, I should properly
        // fingerprint the session_key_raw and set that as the value here
        session_key_raw = session_key_raw.with_key_id("fingerprint");

        let session_key = Arc::new(session_key_raw);

        Ok(Self { session_key })
    }
}
