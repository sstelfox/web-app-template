use std::io::Write;
use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use std::path::PathBuf;

use jwt_simple::prelude::*;
use pico_args::Arguments;
use tracing::Level;

use crate::app::Error;

#[derive(Debug)]
pub struct Config {
    listen_addr: SocketAddr,
    log_level: Level,

    db_url: Option<String>,
    session_key_path: PathBuf,
}

impl Config {
    pub fn db_url(&self) -> Option<&str> {
        self.db_url.as_ref().map(String::as_ref)
    }

    pub fn listen_addr(&self) -> &SocketAddr {
        &self.listen_addr
    }

    pub fn log_level(&self) -> Level {
        self.log_level.clone()
    }

    pub fn parse_cli_arguments() -> Result<Self, Error> {
        let mut args = Arguments::from_env();

        let db_url = args
            .opt_value_from_str("--db-url")?;

        let listen_addr = args
            .opt_value_from_str("--listen")?
            .unwrap_or(SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 3000));

        let log_level = args
            .opt_value_from_str("--log-level")?
            .unwrap_or(Level::INFO);

        let session_key_path: PathBuf = args
            .opt_value_from_str("--session-key")?
            .unwrap_or("./data/session.key".into());

        if args.contains("--generate") {
            tracing::warn!(key_path = ?session_key_path, "generating new session key");

            // don't allow overwriting a key if it already exists
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(session_key_path.clone())
                .map_err(|err| Error::UnwritableSessionKey(err))?;

            let new_key = ES384KeyPair::generate().to_pem().expect("fresh keys to export");
            file.write_all(new_key.as_bytes()).map_err(|err| Error::UnwritableSessionKey(err))?;

            tracing::info!(key_path = ?session_key_path, "new session key generated successfully");
        }

        Ok(Config {
            listen_addr,
            log_level,
            db_url,
            session_key_path,
        })
    }

    pub fn session_key_path(&self) -> &PathBuf {
        &self.session_key_path
    }
}
