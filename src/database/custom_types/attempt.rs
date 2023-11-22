use serde::{Deserialize, Serialize};

use sqlx::encode::IsNull;
use sqlx::error::BoxDynError;
use sqlx::sqlite::{SqliteArgumentValue, SqliteTypeInfo, SqliteValueRef};
use sqlx::{Decode, Encode, Sqlite, Type};

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(transparent)]
pub struct Attempt(usize);

impl Attempt {
    pub fn next(self) -> Self {
        Self(self.0 + 1)
    }

    pub fn zero() -> Self {
        Self(0)
    }
}

impl Decode<'_, Sqlite> for Attempt {
    fn decode(value: SqliteValueRef<'_>) -> Result<Self, BoxDynError> {
        let db_val = <i32 as Decode<Sqlite>>::decode(value)?;

        if db_val < 1 {
            return Err(AttemptError::NonPositiveValue(db_val).into());
        }

        Ok(Self(db_val as usize))
    }
}

impl Encode<'_, Sqlite> for Attempt {
    fn encode_by_ref(&self, args: &mut Vec<SqliteArgumentValue<'_>>) -> IsNull {
        args.push(SqliteArgumentValue::Int(self.0 as i32));
        IsNull::No
    }
}

impl Type<Sqlite> for Attempt {
    fn compatible(ty: &SqliteTypeInfo) -> bool {
        <i32 as Type<Sqlite>>::compatible(ty)
    }

    fn type_info() -> SqliteTypeInfo {
        <i32 as Type<Sqlite>>::type_info()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AttemptError {
    #[error("database contained values that isn't positive: {0}")]
    NonPositiveValue(i32),
}
