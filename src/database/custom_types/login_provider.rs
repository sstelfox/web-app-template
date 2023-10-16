use std::borrow::Cow;
use std::fmt::{self, Display, Formatter};

use serde::{Deserialize, Serialize};
use sqlx::{Decode, Encode, Sqlite, Type};
use sqlx::encode::IsNull;
use sqlx::error::BoxDynError;
use sqlx::sqlite::{SqliteArgumentValue, SqliteTypeInfo, SqliteValueRef};

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
#[serde(rename_all = "snake_case")]
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

    pub fn parse_str(val: &str) -> Result<Self, LoginProviderError> {
        match val {
            "google" => Ok(LoginProvider::Google),
            _ => Err(LoginProviderError::UnknownProvider),
        }
    }
}

impl Decode<'_, Sqlite> for LoginProvider {
    fn decode(value: SqliteValueRef<'_>) -> Result<Self, BoxDynError> {
        let inner_val = <String as Decode<Sqlite>>::decode(value)?;
        Self::parse_str(&inner_val).map_err(Into::into)
    }
}

impl Encode<'_, Sqlite> for LoginProvider {
    fn encode_by_ref(&self, args: &mut Vec<SqliteArgumentValue<'_>>) -> IsNull {
        args.push(SqliteArgumentValue::Text(Cow::Owned(self.to_string())));
        IsNull::No
    }
}

impl Type<Sqlite> for LoginProvider {
    fn compatible(ty: &SqliteTypeInfo) -> bool {
        <String as Type<Sqlite>>::compatible(ty)
    }

    fn type_info() -> SqliteTypeInfo {
        <String as Type<Sqlite>>::type_info()
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

#[derive(Debug, thiserror::Error)]
pub enum LoginProviderError {
    #[error("unknown login provider")]
    UnknownProvider,
}

#[cfg(test)]
mod test {
    use std::error::Error;

    use crate::tests::prelude::*;

    use super::*;

    // SQLx has turned out to be a largely untrustworthy and inconsistent library when it comes to
    // encoding and decoding, as well as mixed support of the actual underlying database. This
    // unfortunately means that I need to test _into_ their interface to ensure they're behaving
    // the way the code in this repository expects.

    #[tokio::test]
    async fn test_sqlx_decoding() {
        let db_pool = test_database().await;
        let mut transact = db_pool.begin().await.expect("transaction");


        // note: UUIDs are stored little-endian in the database, this fixture represents the little
        // endian encoding of the expected_did string above.
        let decoded_login_provider: LoginProvider = sqlx::query_scalar!("SELECT 'google' as 'login_provider: LoginProvider';")
            .fetch_one(&mut *transact)
            .await
            .expect("decode to succeed");
        assert!(matches!(decoded_login_provider, LoginProvider::Google));

        #[derive(sqlx::FromRow)]
        struct LoginProviderTest {
            login_provider: LoginProvider,
        }

        let decoded_obj = sqlx::query_as!(LoginProviderTest, "SELECT 'google' as 'login_provider: LoginProvider';")
            .fetch_one(&mut *transact)
            .await
            .expect("decode to succeed");
        assert!(matches!(decoded_obj.login_provider, LoginProvider::Google));

        transact.rollback().await.expect("rollback")
    }

    #[tokio::test]
    async fn test_sqlx_decoding_failures() {
        let db_pool = test_database().await;
        let mut transact = db_pool.begin().await.expect("transaction");

        let invalid_result = sqlx::query_scalar!("SELECT 'bing' as 'login_provider: LoginProvider';")
            .fetch_one(&mut *transact)
            .await;

        assert!(invalid_result.is_err());

        let err = invalid_result.unwrap_err();
        assert!(matches!(err, sqlx::Error::ColumnDecode { .. }));

        let inner_err = err.source().expect("a source");
        let login_provider_error = inner_err.downcast_ref::<LoginProviderError>().expect("error to be ours");
        assert!(matches!(login_provider_error, LoginProviderError::UnknownProvider));

        transact.rollback().await.expect("rollback")
    }

    #[tokio::test]
    async fn test_sqlx_encoding() {
        let db_pool = test_database().await;
        let mut transact = db_pool.begin().await.expect("transaction");

        sqlx::query("CREATE TABLE login_provider_encoding_test (login_provider TEXT NOT NULL);")
            .execute(&mut *transact)
            .await
            .expect("setup to succeed");

        let sample_login_provider = LoginProvider::Google;
        let returned_login_provider: LoginProvider = sqlx::query_scalar(
            r#"INSERT INTO login_provider_encoding_test (login_provider)
                   VALUES ($1)
                   RETURNING login_provider as 'login_provider: LoginProvider';"#,
        )
        .bind(sample_login_provider)
        .fetch_one(&mut *transact)
        .await
        .expect("insert to succeed");

        assert_eq!(sample_login_provider, returned_login_provider);

        let raw_login_provider: String = sqlx::query_scalar("SELECT login_provider FROM login_provider_encoding_test;")
            .fetch_one(&mut *transact)
            .await
            .expect("return to succeed");

        assert_eq!(&raw_login_provider, &"google");
    }
}
