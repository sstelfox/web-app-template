use std::sync::Arc;

use axum::async_trait;

#[async_trait]
pub trait DataSource {
    async fn ready(&mut self) -> Result<(), DataSourceError>;
}

#[derive(Debug, thiserror::Error)]
pub enum DataSourceError {
}

pub type DynDataSource = Arc<dyn DataSource + Send + Sync>;

pub struct StateDataSource(DynDataSource);
