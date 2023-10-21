#![allow(dead_code)]

use std::net::IpAddr;

const X_REAL_IP_HEADER_KEY: &str = "X-Real-IP";

pub struct ClientIp(IpAddr);
