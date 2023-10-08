use serde::{Deserialize, Serialize};

use crate::database::custom_types::LoginProviderConfig;

static LOGIN_PROVIDER_CONFIGS: phf::Map<u8, LoginProviderConfig> = phf::phf_map! {
    1u8 => LoginProviderConfig::new(
        "https://accounts.google.com/o/oauth2/v2/auth",
        Some("https://www.googleapis.com/oauth2/v3/token"),
        Some("https://oauth2.googleapis.com/revoke"),
        &[
            "https://www.googleapis.com/auth/userinfo.email",
            "https://www.googleapis.com/auth/userinfo.profile"
        ],
    ),
};

#[derive(
    Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, sqlx::Type,
)]
#[serde(rename = "snake_case")]
pub enum LoginProvider {
    Google,
}

impl LoginProvider {
    pub const fn as_str(&self) -> &'static str {
        match &self {
            LoginProvider::Google => "google",
        }
    }

    pub const fn as_u8(&self) -> u8 {
        match &self {
            LoginProvider::Google => 1,
        }
    }

    pub fn config(&self) -> &LoginProviderConfig {
        LOGIN_PROVIDER_CONFIGS
            .get(&self.as_u8())
            .expect("hardcoded configs to be present")
    }
}

impl From<String> for LoginProvider {
    fn from(val: String) -> Self {
        match val.as_str() {
            "google" => LoginProvider::Google,
            _ => panic!("attempted to access unknown provider"),
        }
    }
}
