mod api_key_identity;
mod database;
mod secrets;
mod session_identity;

pub use api_key_identity::ApiKeyIdentity;
pub use session_identity::SessionIdentity;

pub static LOGIN_PATH: &str = "/auth/login";

pub static SESSION_COOKIE_NAME: &str = "_session_id";
