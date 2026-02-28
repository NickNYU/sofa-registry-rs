use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sofa_registry_core::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistenceData {
    pub data_center: String,
    pub data_key: String,
    pub data_value: String,
    pub version: i64,
}

#[async_trait]
pub trait ProvideDataRepository: Send + Sync {
    async fn put(&self, data: PersistenceData) -> Result<bool>;

    async fn get(&self, data_center: &str, data_key: &str) -> Result<Option<PersistenceData>>;

    async fn remove(&self, data_center: &str, data_key: &str) -> Result<bool>;

    async fn get_all(&self, data_center: &str) -> Result<Vec<PersistenceData>>;
}
