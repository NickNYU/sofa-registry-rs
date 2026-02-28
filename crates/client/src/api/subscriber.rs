use sofa_registry_core::model::ReceivedData;
use std::sync::Arc;

use super::observer::SubscriberDataObserver;

/// Handle returned after registering a subscriber.
///
/// Allows peeking at the latest data or attaching an observer.
pub trait SubscriberHandle: Send + Sync {
    /// Get the latest received data snapshot, if any.
    fn peek_data(&self) -> Option<ReceivedData>;

    /// Attach an observer that will be called when data changes.
    fn set_observer(&self, observer: Arc<dyn SubscriberDataObserver>);

    /// The data ID this subscriber is watching.
    fn data_id(&self) -> &str;

    /// Whether this subscriber is currently registered.
    fn is_registered(&self) -> bool;
}
