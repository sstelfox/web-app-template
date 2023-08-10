use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use std::path::PathBuf;

use pico_args::Arguments;
use tracing::Level;

use crate::app::Error;

#[derive(Debug)]
pub struct Config {
    listen_addr: SocketAddr,
    log_level: Level,

    db_url: Option<String>,
    jwt_key_path: Option<PathBuf>,
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

        let jwt_key_path = args
            .opt_value_from_str("--session-key")?;

        Ok(Config {
            listen_addr,
            log_level,
            db_url,
            jwt_key_path,
        })
    }

    pub fn jwt_key_path(&self) -> Option<&PathBuf> {
        self.jwt_key_path.as_ref()
    }
}
