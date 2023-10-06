mod api_key_identity;
mod client_ip;
mod database;
mod server_base;
mod session_identity;

pub use api_key_identity::ApiKeyIdentity;
pub use client_ip::ClientIp;
pub use server_base::ServerBase;
pub use session_identity::SessionIdentity;

pub static LOGIN_PATH: &str = "/auth/login";

pub static SESSION_COOKIE_NAME: &str = "_session_id";
