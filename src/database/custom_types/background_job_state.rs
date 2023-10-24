use std::fmt::{self, Display, Formatter};

use sqlx::encode::IsNull;
use sqlx::error::BoxDynError;
use sqlx::sqlite::{SqliteArgumentValue, SqliteTypeInfo, SqliteValueRef};
use sqlx::{Decode, Encode, Sqlite, Type};

#[derive(Clone, Copy, Debug)]
pub enum BackgroundJobState {
    Scheduled,
    Cancelled,
    Complete,
    Dead,
}

impl Decode<'_, Sqlite> for BackgroundJobState {
    fn decode(value: SqliteValueRef<'_>) -> Result<Self, BoxDynError> {
        let inner_val = <&str as Decode<Sqlite>>::decode(value)?;
        Self::try_from(inner_val).map_err(Into::into)
    }
}

impl Encode<'_, Sqlite> for BackgroundJobState {
    fn encode_by_ref(&self, args: &mut Vec<SqliteArgumentValue<'_>>) -> IsNull {
        args.push(SqliteArgumentValue::Text(self.to_string().into()));
        IsNull::No
    }
}

impl Type<Sqlite> for BackgroundJobState {
    fn compatible(ty: &SqliteTypeInfo) -> bool {
        <&str as Type<Sqlite>>::compatible(ty)
    }

    fn type_info() -> SqliteTypeInfo {
        <&str as Type<Sqlite>>::type_info()
    }
}

impl Display for BackgroundJobState {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let msg = match self {
            BackgroundJobState::Scheduled => "scheduled",
            BackgroundJobState::Cancelled => "cancelled",
            BackgroundJobState::Complete => "complete",
            BackgroundJobState::Dead => "dead",
        };

        f.write_str(msg)
    }
}

impl TryFrom<&str> for BackgroundJobState {
    type Error = BackgroundJobStateError;

    fn try_from(val: &str) -> Result<Self, BackgroundJobStateError> {
        let variant = match val {
            "scheduled" => BackgroundJobState::Scheduled,
            "cancelled" => BackgroundJobState::Cancelled,
            "complete" => BackgroundJobState::Complete,
            "dead" => BackgroundJobState::Dead,
            _ => return Err(BackgroundJobStateError::InvalidStateValue),
        };

        Ok(variant)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BackgroundJobStateError {
    #[error("attempted to decode unknown state value")]
    InvalidStateValue,
}
