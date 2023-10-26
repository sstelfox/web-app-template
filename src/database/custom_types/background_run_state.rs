use std::fmt::{self, Display, Formatter};

use sqlx::encode::IsNull;
use sqlx::error::BoxDynError;
use sqlx::sqlite::{SqliteArgumentValue, SqliteTypeInfo, SqliteValueRef};
use sqlx::{Decode, Encode, Sqlite, Type};

#[derive(Clone, Copy)]
pub enum BackgroundRunState {
    Running,
    Completed,
    Cancelled,
    Errored,
    TimedOut,
    Panicked,
}

impl Decode<'_, Sqlite> for BackgroundRunState {
    fn decode(value: SqliteValueRef<'_>) -> Result<Self, BoxDynError> {
        let inner_val = <&str as Decode<Sqlite>>::decode(value)?;
        Self::try_from(inner_val).map_err(Into::into)
    }
}

impl Encode<'_, Sqlite> for BackgroundRunState {
    fn encode_by_ref(&self, args: &mut Vec<SqliteArgumentValue<'_>>) -> IsNull {
        args.push(SqliteArgumentValue::Text(self.to_string().into()));
        IsNull::No
    }
}

impl Type<Sqlite> for BackgroundRunState {
    fn compatible(ty: &SqliteTypeInfo) -> bool {
        <&str as Type<Sqlite>>::compatible(ty)
    }

    fn type_info() -> SqliteTypeInfo {
        <&str as Type<Sqlite>>::type_info()
    }
}

impl Display for BackgroundRunState {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let msg = match self {
            BackgroundRunState::Running => "running",
            BackgroundRunState::Completed => "completed",
            BackgroundRunState::Cancelled => "cancelled",
            BackgroundRunState::Errored => "errored",
            BackgroundRunState::TimedOut => "timed_out",
            BackgroundRunState::Panicked => "panicked",
        };

        f.write_str(msg)
    }
}

impl TryFrom<&str> for BackgroundRunState {
    type Error = BackgroundRunStateError;

    fn try_from(val: &str) -> Result<Self, BackgroundRunStateError> {
        let variant = match val {
            "running" => BackgroundRunState::Running,
            "completed" => BackgroundRunState::Completed,
            "cancelled" => BackgroundRunState::Cancelled,
            "errored" => BackgroundRunState::Errored,
            "timed_out" => BackgroundRunState::TimedOut,
            "panicked" => BackgroundRunState::Panicked,
            _ => return Err(BackgroundRunStateError::InvalidStateType(val.to_string())),
        };

        Ok(variant)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BackgroundRunStateError {
    #[error("attempted to decode unknown background run state type '{0}'")]
    InvalidStateType(String),
}
