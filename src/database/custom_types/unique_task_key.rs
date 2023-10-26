use serde::{Deserialize, Serialize};

use crate::background_jobs::JobStoreError;
use crate::database::custom_types::DbBool;

#[derive(Deserialize, Serialize, sqlx::Type)]
#[serde(transparent)]
#[sqlx(transparent)]
pub struct UniqueTaskKey(String);

impl UniqueTaskKey {
    pub async fn is_active(
        &self,
        conn: &mut sqlx::SqliteConnection,
    ) -> Result<bool, UniqueTaskKeyError> {
        sqlx::query_scalar!(
            r#"SELECT COALESCE((
                   SELECT 1 FROM background_jobs
                       WHERE unique_key = $1 AND state IN ('scheduled', 'active')
                       LIMIT 1
               ), 0) AS 'exists!: DbBool';"#,
            self,
        )
        .fetch_one(&mut *conn)
        .await
        .map(|r| r.into())
        .map_err(UniqueTaskKeyError::ActiveLookupFailed)
    }
}

impl From<&str> for UniqueTaskKey {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum UniqueTaskKeyError {
    #[error("failed to check whether unique key is active")]
    ActiveLookupFailed(sqlx::Error),
}

impl From<UniqueTaskKeyError> for JobStoreError {
    fn from(value: UniqueTaskKeyError) -> Self {
        JobStoreError::DataCorruption(value.into())
    }
}
