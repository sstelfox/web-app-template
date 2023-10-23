use std::fmt::{self, Display, Formatter};

use sqlx::encode::IsNull;
use sqlx::error::BoxDynError;
use sqlx::sqlite::{SqliteArgumentValue, SqliteTypeInfo, SqliteValueRef};
use sqlx::{Decode, Encode, Sqlite, Type};

#[derive(Clone, Copy)]
pub enum JobRunResult {
    Panic,
    TimedOut,
    Error,
    Success,
}

impl Decode<'_, Sqlite> for JobRunResult {
    fn decode(value: SqliteValueRef<'_>) -> Result<Self, BoxDynError> {
        let inner_val = <&str as Decode<Sqlite>>::decode(value)?;
        Self::try_from(inner_val).map_err(Into::into)
    }
}

impl Encode<'_, Sqlite> for JobRunResult {
    fn encode_by_ref(&self, args: &mut Vec<SqliteArgumentValue<'_>>) -> IsNull {
        args.push(SqliteArgumentValue::Text(self.to_string().into()));
        IsNull::No
    }
}

impl Type<Sqlite> for JobRunResult {
    fn compatible(ty: &SqliteTypeInfo) -> bool {
        <&str as Type<Sqlite>>::compatible(ty)
    }

    fn type_info() -> SqliteTypeInfo {
        <&str as Type<Sqlite>>::type_info()
    }
}

impl Display for JobRunResult {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let msg = match self {
            JobRunResult::Panic => "panic",
            JobRunResult::TimedOut => "timed_out",
            JobRunResult::Error => "error",
            JobRunResult::Success => "success",
        };

        f.write_str(msg)
    }
}

impl TryFrom<&str> for JobRunResult {
    type Error = JobRunResultError;

    fn try_from(val: &str) -> Result<Self, JobRunResultError> {
        let variant = match val {
            "panic" => JobRunResult::Panic,
            "timed_out" => JobRunResult::TimedOut,
            "error" => JobRunResult::Error,
            "success" => JobRunResult::Success,
            _ => return Err(JobRunResultError::InvalidResultType),
        };

        Ok(variant)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum JobRunResultError {
    #[error("attempted to decode unknown result type")]
    InvalidResultType,
}
