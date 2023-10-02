use std::net::SocketAddr;
use std::path::PathBuf;

use pico_args::Arguments;
use tracing::Level;
use url::Url;

use crate::app::{Error, Version};

#[derive(Debug)]
pub struct Config {
    listen_addr: SocketAddr,
    log_level: Level,

    db_url: Url,
    smtp_url: Option<Url>,

    google_client_id: String,
    google_client_secret: String,

    session_key_path: PathBuf,
}

impl Config {
    pub fn db_url(&self) -> Url {
        self.db_url.clone()
    }

    pub fn listen_addr(&self) -> &SocketAddr {
        &self.listen_addr
    }

    pub fn log_level(&self) -> Level {
        self.log_level.clone()
    }

    pub fn smtp_url(&self) -> Option<Url> {
        self.smtp_url.as_ref().map(|u| u.clone())
    }

    pub fn from_env_and_args() -> Result<Self, Error> {
        dotenvy::dotenv().map_err(|err| Error::EnvironmentUnavailable(err))?;
        let mut cli_args = Arguments::from_env();

        if cli_args.contains("-h") || cli_args.contains("--help") {
            print_help();
            std::process::exit(0);
        }

        if cli_args.contains("-v") || cli_args.contains("--version") {
            print_version();
            std::process::exit(0);
        }

        let db_str = match cli_args.opt_value_from_str("--db")? {
            Some(du) => du,
            None => match std::env::var("DATABASE_URL") {
                Ok(du) if !du.is_empty() => du,
                _ => "sqlite://:memory:".to_string(),
            },
        };
        let db_url = Url::parse(&db_str).map_err(|err| Error::InvalidDatabaseUrl(err))?;

        let smtp_str = match cli_args.opt_value_from_str("--smtp")? {
            Some(du) => Some(du),
            None => match std::env::var("SMTP_URL") {
                Ok(su) if !su.is_empty() => Some(su),
                _ => None,
            },
        };
        let smtp_url = match smtp_str {
            Some(su) => Some(Url::parse(&su).map_err(|err| Error::InvalidSmtpUrl(err))?),
            None => None,
        };

        let session_key_str = match cli_args.opt_value_from_str("--session-key")? {
            Some(path) => path,
            None => match std::env::var("SESSION_KEY") {
                Ok(sk) if !sk.is_empty() => sk,
                _ => "./data/session.key".to_string(),
            }
        };
        let session_key_path = PathBuf::from(session_key_str);

        let google_client_id = match std::env::var("GOOGLE_OAUTH_CLIENT_ID") {
            Ok(cid) if !cid.is_empty() => cid,
            _ => return Err(Error::MissingGoogleClientId),
        };
        let google_client_secret = match std::env::var("GOOGLE_OAUTH_CLIENT_SECRET") {
            Ok(cs) if !cs.is_empty() => cs,
            _ => return Err(Error::MissingGoogleClientSecret),
        };

        let listen_str = match cli_args.opt_value_from_str("--listen")? {
            Some(l) => l,
            None => match std::env::var("LISTEN_ADDR") {
                Ok(l) if !l.is_empty() => l,
                _ => "[::]:3000".to_string(),
            },
        };
        let listen_addr: SocketAddr = listen_str
            .parse()
            .map_err(|err| Error::InvalidListenAddr(err))?;

        let log_level = cli_args
            .opt_value_from_str("--log-level")?
            .unwrap_or(Level::INFO);

        Ok(Config {
            listen_addr,
            log_level,

            db_url,
            smtp_url,

            google_client_id,
            google_client_secret,

            session_key_path,
        })
    }

    pub fn session_key_path(&self) -> &PathBuf {
        &self.session_key_path
    }
}

fn print_help() {
    println!("Service may be configured using the environment or CLI flags\n");
    println!("  Available options:");
    println!("    -h, --help                    Print this notice and exit");
    println!("    -v, --version                 Display the version of this compiled version");
    println!("                                  and exit\n");
    println!("    --listen, LISTEN_ADDR Specify the address to bind to (default [::]:3000)");
    println!("    --session-key, SESSION_KEY    Path to the p384 private key used for session");
    println!("                                  key generation and verification\n");
    println!("    --db, DATABASE_URL            Configure the url and settings of the sqlite");
    println!("                                  database (default in memory)");
    println!("    --smtp, SMTP_URL              Configure the url and settings of the SMTP");
    println!("                                  relay server (default print to stdout)");
    println!("  Additional Environment Options:");
    println!("    GOOGLE_OAUTH_CLIENT_ID        The client ID associated with this app for");
    println!("                                  performing authentication using Google services.");
    println!("    GOOGLE_OAUTH_CLIENT_SECRET    The client secret paired with the client ID.");
}

fn print_version() {
    let version = Version::new();

    println!(
        "Service version {} built in {} mode with features: {:?}",
        version.version, version.build_profile, version.features
    );
}
