use std::io::Write;

use axum::extract::FromRef;
use jwt_simple::algorithms::{ECDSAP384KeyPairLike, ES384KeyPair};
use oauth2::basic::BasicClient;
use oauth2::{AuthUrl, ClientId, ClientSecret, RedirectUrl, TokenUrl};
use sha2::Digest;

use crate::app::{Config, Error, Hostname, SessionCreationKey, SessionVerificationKey};
use crate::database::{self, Database};

static GOOGLE_OAUTH_AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";

static GOOGLE_OAUTH_TOKEN_URL: &str = "https://www.googleapis.com/oauth2/v3/token";

static GOOGLE_CALLBACK_PATH: &str = "/auth/callback/google";

#[derive(Clone)]
pub struct OAuthClient(BasicClient);

#[derive(Clone)]
pub struct State {
    hostname: Hostname,

    database: Database,
    oauth_client: OAuthClient,

    session_key: SessionCreationKey,
    session_verifier: SessionVerificationKey,
}

impl State {
    // not implemented as a From trait so it can be async
    pub async fn from_config(config: &Config) -> Result<Self, Error> {
        let hostname = Hostname(config.hostname());

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
            hostname,

            database,
            oauth_client: oauth_client(config)?,

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

impl FromRef<State> for Hostname {
    fn from_ref(state: &State) -> Self {
        state.hostname.clone()
    }
}

impl FromRef<State> for OAuthClient {
    fn from_ref(state: &State) -> Self {
        state.oauth_client.clone()
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

fn oauth_client(config: &Config) -> Result<OAuthClient, Error> {
    let auth_url =
        AuthUrl::new(GOOGLE_OAUTH_AUTH_URL.to_string()).expect("static auth url to be valid");
    let token_url =
        TokenUrl::new(GOOGLE_OAUTH_TOKEN_URL.to_string()).expect("static token url to be valid");

    let mut redirect_url = config.hostname();
    redirect_url.set_path(GOOGLE_CALLBACK_PATH);
    let redirect_url = RedirectUrl::from_url(redirect_url);

    let client = BasicClient::new(
        ClientId::new(config.google_client_id().to_string()),
        Some(ClientSecret::new(config.google_client_secret().to_string())),
        auth_url,
        Some(token_url),
    )
    .set_redirect_uri(redirect_url);

    Ok(OAuthClient(client))
}
