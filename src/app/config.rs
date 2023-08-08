use pico_args::Arguments;
use std::net::{IpAddr, Ipv6Addr, SocketAddr};

use crate::app::Error;

#[derive(Debug)]
pub struct Config {
    listen_addr: SocketAddr,
}

impl Config {
    pub fn listen_addr(&self) -> &SocketAddr {
        &self.listen_addr
    }

    pub fn parse_cli_arguments() -> Result<Self, Error> {
        let mut args = Arguments::from_env();

        let listen_addr = args
            .opt_value_from_str("--listen")?
            .unwrap_or(SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 3000));

        Ok(Config {
            listen_addr,
        })
    }
}
