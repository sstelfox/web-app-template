use serde::{Deserialize, Serialize};

use crate::background_jobs::JobStoreError;
use crate::database::custom_types::BackgroundJobId;
use crate::database::DatabaseConnection;

#[derive(Deserialize, Serialize, sqlx::Type)]
#[serde(transparent)]
#[sqlx(transparent)]
pub struct UniqueTaskKey(String);

impl UniqueTaskKey {
    pub async fn existing(
        &self,
        conn: &mut DatabaseConnection,
    ) -> Result<Option<BackgroundJobId>, UniqueTaskKeyError> {
        sqlx::query_scalar!(
            r#"SELECT id as 'id: BackgroundJobId' FROM background_jobs
                   WHERE unique_key = $1 AND state IN ('scheduled', 'active')
                   LIMIT 1;"#,
            self,
        )
        .fetch_optional(&mut *conn)
        .await
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
