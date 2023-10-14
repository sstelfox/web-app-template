use sqlx::{Decode, Encode, Sqlite, Type};
use sqlx::encode::IsNull;
use sqlx::error::BoxDynError;
use sqlx::sqlite::{SqliteArgumentValue, SqliteTypeInfo, SqliteValueRef};

pub enum BackgroundJobState {
    New,
    Started,
    Retrying,
    Cancelled,
    Failed,
    Complete,
}

impl BackgroundJobState {
    pub fn as_i32(&self) -> i32 {
        match &self {
            BackgroundJobState::New => 0,
            BackgroundJobState::Started => 1,
            BackgroundJobState::Retrying => 2,
            BackgroundJobState::Cancelled => 3,
            BackgroundJobState::Failed => 4,
            BackgroundJobState::Complete => 5,
        }
    }

    pub fn from_u16(val: u16) -> Result<Self, BackgroundJobStateError> {
        let variant = match val {
            0 => BackgroundJobState::New,
            1 => BackgroundJobState::Started,
            2 => BackgroundJobState::Retrying,
            3 => BackgroundJobState::Cancelled,
            4 => BackgroundJobState::Failed,
            5 => BackgroundJobState::Complete,
            _ => return Err(BackgroundJobStateError::InvalidStateValue),
        };

        Ok(variant)
    }
}

impl Encode<'_, Sqlite> for BackgroundJobState {
    fn encode_by_ref(&self, args: &mut Vec<SqliteArgumentValue<'_>>) -> IsNull {
        args.push(SqliteArgumentValue::Int(self.as_i32()));
        IsNull::No
    }
}

impl Decode<'_, Sqlite> for BackgroundJobState {
    fn decode(value: SqliteValueRef<'_>) -> Result<Self, BoxDynError> {
        let inner_val = <u16 as Decode<Sqlite>>::decode(value)?;
        Self::from_u16(inner_val).map_err(Into::into)
    }
}

impl Type<Sqlite> for BackgroundJobState {
    fn compatible(ty: &SqliteTypeInfo) -> bool {
        <u16 as Type<Sqlite>>::compatible(ty)
    }

    fn type_info() -> SqliteTypeInfo {
        <u16 as Type<Sqlite>>::type_info()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BackgroundJobStateError {
    #[error("attempted to decode unknown state number")]
    InvalidStateValue,
}
