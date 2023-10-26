use sqlx::encode::IsNull;
use sqlx::error::BoxDynError;
use sqlx::sqlite::{SqliteArgumentValue, SqliteTypeInfo, SqliteValueRef};
use sqlx::{Decode, Encode, Sqlite, Type};

pub struct DbBool(bool);

impl Decode<'_, Sqlite> for DbBool {
    fn decode(value: SqliteValueRef<'_>) -> Result<Self, BoxDynError> {
        let inner_val = <i32 as Decode<Sqlite>>::decode(value)?;

        match inner_val {
            0 => Ok(Self(false)),
            1 => Ok(Self(true)),
            _ => Err(DbBoolError::BadColumnData(inner_val).into()),
        }
    }
}

impl Encode<'_, Sqlite> for DbBool {
    fn encode_by_ref(&self, args: &mut Vec<SqliteArgumentValue<'_>>) -> IsNull {
        let inner_val = if self.0 { 1 } else { 0 };

        args.push(SqliteArgumentValue::Int(inner_val));
        IsNull::No
    }
}

impl Type<Sqlite> for DbBool {
    fn compatible(ty: &SqliteTypeInfo) -> bool {
        <i32 as Type<Sqlite>>::compatible(ty)
    }

    fn type_info() -> SqliteTypeInfo {
        <i32 as Type<Sqlite>>::type_info()
    }
}

impl From<DbBool> for bool {
    fn from(value: DbBool) -> bool {
        value.0
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DbBoolError {
    #[error("column contained value other than 0 or 1: {0}")]
    BadColumnData(i32),
}
