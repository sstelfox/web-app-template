use sqlx::{Decode, Encode, Sqlite, Type};
use sqlx::encode::IsNull;
use sqlx::error::BoxDynError;
use sqlx::sqlite::{SqliteArgumentValue, SqliteTypeInfo, SqliteValueRef};

#[derive(Clone, Copy)]
pub enum BackgroundJobState {
    New,
    Started,
    Retrying,
    Cancelled,
    Failed,
    Complete,
}

impl Encode<'_, Sqlite> for BackgroundJobState {
    fn encode_by_ref(&self, args: &mut Vec<SqliteArgumentValue<'_>>) -> IsNull {
        args.push(SqliteArgumentValue::Int((*self).into()));
        IsNull::No
    }
}

impl Decode<'_, Sqlite> for BackgroundJobState {
    fn decode(value: SqliteValueRef<'_>) -> Result<Self, BoxDynError> {
        let inner_val = <i32 as Decode<Sqlite>>::decode(value)?;
        Self::try_from(inner_val).map_err(Into::into)
    }
}

impl Type<Sqlite> for BackgroundJobState {
    fn compatible(ty: &SqliteTypeInfo) -> bool {
        <i32 as Type<Sqlite>>::compatible(ty)
    }

    fn type_info() -> SqliteTypeInfo {
        <i32 as Type<Sqlite>>::type_info()
    }
}

impl From<BackgroundJobState> for i32 {
    fn from(val: BackgroundJobState) -> Self {
        match val {
            BackgroundJobState::New => 0,
            BackgroundJobState::Started => 1,
            BackgroundJobState::Retrying => 2,
            BackgroundJobState::Cancelled => 3,
            BackgroundJobState::Failed => 4,
            BackgroundJobState::Complete => 5,
        }
    }
}

impl TryFrom<i32> for BackgroundJobState {
    type Error = BackgroundJobStateError;

    fn try_from(val: i32) -> Result<Self, Self::Error> {
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

#[derive(Debug, thiserror::Error)]
pub enum BackgroundJobStateError {
    #[error("attempted to decode unknown state number")]
    InvalidStateValue,
}
