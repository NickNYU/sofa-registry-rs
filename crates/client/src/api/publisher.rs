use async_trait::async_trait;
use sofa_registry_core::error::Result;

/// Handle returned after registering a publisher.
///
/// Allows republishing new data or unregistering.
#[async_trait]
pub trait PublisherHandle: Send + Sync {
    /// Publish new data values, replacing any previously published data.
    async fn republish(&self, data: &[&str]) -> Result<()>;

    /// Remove this publisher from the registry.
    async fn unregister(&self) -> Result<()>;

    /// The data ID this publisher is registered under.
    fn data_id(&self) -> &str;

    /// The unique registration ID assigned by the server.
    fn regist_id(&self) -> &str;

    /// Whether this publisher is currently registered.
    fn is_registered(&self) -> bool;
}
