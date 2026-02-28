use async_trait::async_trait;
use sofa_registry_core::error::Result;
use std::sync::Arc;

use super::publisher::PublisherHandle;
use super::registration::{PublisherRegistration, SubscriberRegistration};
use super::subscriber::SubscriberHandle;

/// Top-level interface for interacting with the SOFARegistry service.
#[async_trait]
pub trait RegistryClient: Send + Sync {
    /// Register as a publisher for the given data ID and publish initial data.
    async fn register_publisher(
        &self,
        reg: PublisherRegistration,
        data: &[&str],
    ) -> Result<Arc<dyn PublisherHandle>>;

    /// Register as a subscriber for the given data ID.
    async fn register_subscriber(
        &self,
        reg: SubscriberRegistration,
    ) -> Result<Arc<dyn SubscriberHandle>>;
}
