use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sofa_registry_core::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientManagerAddress {
    pub data_center: String,
    pub address: String,
    pub operation: String,
    pub gmt_create: DateTime<Utc>,
}

#[async_trait]
pub trait ClientManagerAddressRepository: Send + Sync {
    async fn get_client_off_addresses(
        &self,
        data_center: &str,
    ) -> Result<Vec<ClientManagerAddress>>;

    async fn client_off(&self, data_center: &str, address: &str) -> Result<bool>;

    async fn client_on(&self, data_center: &str, address: &str) -> Result<bool>;
}
