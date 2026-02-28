use sofa_registry_core::model::ReceivedData;

/// Callback invoked when subscribed data changes on the registry.
pub trait SubscriberDataObserver: Send + Sync {
    fn handle_data(&self, data_id: &str, data: ReceivedData);
}

/// Observer backed by a closure.
pub struct FnObserver<F>
where
    F: Fn(&str, ReceivedData) + Send + Sync,
{
    f: F,
}

impl<F> SubscriberDataObserver for FnObserver<F>
where
    F: Fn(&str, ReceivedData) + Send + Sync,
{
    fn handle_data(&self, data_id: &str, data: ReceivedData) {
        (self.f)(data_id, data);
    }
}

/// Create a [`SubscriberDataObserver`] from a closure.
pub fn observer_fn<F>(f: F) -> FnObserver<F>
where
    F: Fn(&str, ReceivedData) + Send + Sync,
{
    FnObserver { f }
}
