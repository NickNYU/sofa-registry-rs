use async_trait::async_trait;
use sofa_registry_core::error::Result;

#[async_trait]
pub trait InterfaceAppsRepository: Send + Sync {
    async fn get_app_names(&self, data_center: &str, interface_name: &str) -> Result<Vec<String>>;

    async fn register(&self, data_center: &str, app_name: &str, interface_name: &str)
        -> Result<()>;
}
