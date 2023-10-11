use std::borrow::Cow;
use std::fmt::{self, Display, Formatter};

use serde::{Deserialize, Serialize};
use sqlx::{Decode, Encode, Sqlite, Type};
use sqlx::encode::IsNull;
use sqlx::error::BoxDynError;
use sqlx::sqlite::{SqliteArgumentValue, SqliteTypeInfo, SqliteType, SqliteValueRef};

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

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename = "snake_case")]
pub enum LoginProvider {
    Google,
}

impl LoginProvider {
    pub fn as_u8(&self) -> u8 {
        match &self {
            LoginProvider::Google => 1,
        }
    }

    pub fn config(&self) -> &LoginProviderConfig {
        LOGIN_PROVIDER_CONFIGS
            .get(&self.as_u8())
            .expect("hardcoded configs to be present")
    }

    pub fn parse_str(val: &str) -> Result<Self, &str> {
        match val {
            "google" => Ok(LoginProvider::Google),
            _ => Err("unknown login provider"),
        }
    }
}

impl Encode<'_, Sqlite> for LoginProvider {
    fn encode_by_ref(&self, args: &mut Vec<SqliteArgumentValue<'_>>) -> IsNull {
        args.push(SqliteArgumentValue::Text(Cow::Owned(self.to_string())));
        IsNull::No
    }
}

impl Decode<'_, Sqlite> for LoginProvider {
    fn decode(value: SqliteValueRef<'_>) -> Result<Self, BoxDynError> {
        Self::parse_str(&value.text()?).map_err(Into::into)
    }
}

impl Type<Sqlite> for LoginProvider {
    fn compatible(ty: &SqliteTypeInfo) -> bool {
        matches!(ty.0, SqliteType::Text)
    }

    fn type_info() -> SqliteTypeInfo {
        SqliteTypeInfo(SqliteType::Text)
    }
}

impl Display for LoginProvider {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let msg = match &self {
            LoginProvider::Google => "google",
        };

        f.write_str(msg)
    }
}

impl From<String> for LoginProvider {
    fn from(val: String) -> Self {
        Self::parse_str(val.as_str()).expect("valid login provider type")
    }
}
