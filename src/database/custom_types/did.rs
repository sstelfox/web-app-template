use std::borrow::Cow;
use std::fmt::{self, Debug, Display, Formatter};
use std::ops::Deref;

use uuid::Uuid;
use sqlx::{Decode, Encode, Sqlite, Type};
use sqlx::encode::IsNull;
use sqlx::error::BoxDynError;
use sqlx::sqlite::{SqliteArgumentValue, SqliteTypeInfo, SqliteValueRef};

#[derive(Clone, Copy, PartialEq)]
pub struct Did(Uuid);

impl Debug for Did {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

impl Deref for Did {
    type Target = Uuid;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for Did {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for Did {
    fn from(val: Uuid) -> Self {
        Self(val)
    }
}

impl Encode<'_, Sqlite> for Did {
    fn encode_by_ref(&self, args: &mut Vec<SqliteArgumentValue<'_>>) -> IsNull {
        let encoded_bytes = self.0.to_bytes_le();
        args.push(SqliteArgumentValue::Blob(Cow::Owned(encoded_bytes.to_vec())));
        IsNull::No
    }
}

impl Decode<'_, Sqlite> for Did {
    fn decode(value: SqliteValueRef<'_>) -> Result<Self, BoxDynError> {
        let inner_val = <Vec<u8> as Decode<Sqlite>>::decode(value)?;

        if inner_val.len() != 16 {
            return Err(DidError::CorruptSize.into());
        }

        let mut fixed_bytes = [0u8; 16];
        fixed_bytes.copy_from_slice(&inner_val);

        Ok(Self(Uuid::from_bytes_le(fixed_bytes)))
    }
}

impl Type<Sqlite> for Did {
    fn compatible(ty: &SqliteTypeInfo) -> bool {
        <Vec<u8> as Type<Sqlite>>::compatible(ty)
    }

    fn type_info() -> SqliteTypeInfo {
        <Vec<u8> as Type<Sqlite>>::type_info()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DidError {
    #[error("the UUID representation doesn't contain the correct number of bytes")]
    CorruptSize,

    #[error("the provided UUID was not formatted correctly")]
    InvalidUuid(uuid::Error),
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

        let expected_did = Did::from(Uuid::parse_str("c97dc8dd-244f-4465-aab2-9562ba2a128b").expect("uuid"));

        // note: UUIDs are stored little-endian in the database, this fixture represents the little
        // endian encoding of the expected_did string above.
        let decoded_did: Did = sqlx::query_scalar!("SELECT CAST(X'ddc87dc94f246544aab29562ba2a128b' AS BLOB) as 'did: Did';")
            .fetch_one(&mut *transact)
            .await
            .expect("decode to succeed");
        assert_eq!(decoded_did, expected_did);

        #[derive(sqlx::FromRow)]
        struct DidTest {
            did: Did,
        }

        let decoded_obj = sqlx::query_as!(DidTest, "SELECT CAST(X'ddc87dc94f246544aab29562ba2a128b' AS BLOB) as 'did: Did';")
            .fetch_one(&mut *transact)
            .await
            .expect("decode to succeed");
        assert_eq!(decoded_obj.did, expected_did);

        transact.rollback().await.expect("rollback")
    }

    #[tokio::test]
    async fn test_sqlx_decoding_failures() {
        let db_pool = test_database().await;
        let mut transact = db_pool.begin().await.expect("transaction");

        let short_result = sqlx::query_scalar!("SELECT CAST(X'001122334455668899aabbccddeeff' AS BLOB) as 'did: Did';")
            .fetch_one(&mut *transact)
            .await;

        assert!(short_result.is_err());

        let err = short_result.unwrap_err();
        assert!(matches!(err, sqlx::Error::ColumnDecode { .. }));

        let inner_err = err.source().expect("a source");
        let did_error = inner_err.downcast_ref::<DidError>().expect("error to be ours");
        assert!(matches!(did_error, DidError::CorruptSize));

        let long_result = sqlx::query_scalar!("SELECT CAST(X'0011223344556670078899aabbccddeeff' AS BLOB) as 'did: Did';")
            .fetch_one(&mut *transact)
            .await;

        assert!(long_result.is_err());

        let err = long_result.unwrap_err();
        assert!(matches!(err, sqlx::Error::ColumnDecode { .. }));

        let inner_err = err.source().expect("a source");
        let did_error = inner_err.downcast_ref::<DidError>().expect("error to be ours");
        assert!(matches!(did_error, DidError::CorruptSize));

        transact.rollback().await.expect("rollback")
    }

    #[tokio::test]
    async fn test_sqlx_encoding() {
        let db_pool = test_database().await;
        let mut transact = db_pool.begin().await.expect("transaction");

        sqlx::query("CREATE TABLE did_encoding_test (did BLOB NOT NULL);")
            .execute(&mut *transact)
            .await
            .expect("setup to succeed");

        let sample_did = Did::from(Uuid::parse_str("c97dc8dd-244f-4465-aab2-9562ba2a128b").expect("uuid"));
        let returned_did: Did = sqlx::query_scalar(
            "INSERT INTO did_encoding_test (did) VALUES ($1) RETURNING did as 'did: Did';",
        )
        .bind(sample_did)
        .fetch_one(&mut *transact)
        .await
        .expect("insert to succeed");

        assert_eq!(sample_did, returned_did);

        let raw_did: Vec<u8> = sqlx::query_scalar("SELECT did FROM did_encoding_test;")
            .fetch_one(&mut *transact)
            .await
            .expect("return to succeed");

        assert_eq!(&raw_did, &[0xdd, 0xc8, 0x7d, 0xc9, 0x4f, 0x24, 0x65, 0x44, 0xaa, 0xb2, 0x95, 0x62, 0xba, 0x2a, 0x12, 0x8b]);
    }
}
