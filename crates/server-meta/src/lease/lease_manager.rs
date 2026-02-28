use std::sync::Arc;
use std::time::{Duration, Instant};
use dashmap::DashMap;
use parking_lot::RwLock;
use tracing::warn;

/// Generic lease for a node (session or data server)
#[derive(Debug, Clone)]
pub struct Lease<T: Clone> {
    pub node: T,
    pub registered_at: Instant,
    pub last_renewed: Instant,
    pub duration: Duration,
}

impl<T: Clone> Lease<T> {
    pub fn new(node: T, duration: Duration) -> Self {
        let now = Instant::now();
        Self {
            node,
            registered_at: now,
            last_renewed: now,
            duration,
        }
    }

    pub fn renew(&mut self) {
        self.last_renewed = Instant::now();
    }

    pub fn is_expired(&self) -> bool {
        self.last_renewed.elapsed() > self.duration
    }

    pub fn remaining(&self) -> Duration {
        self.duration.checked_sub(self.last_renewed.elapsed()).unwrap_or(Duration::ZERO)
    }
}

/// Observer for lease lifecycle events
pub trait LeaseObserver<T: Clone>: Send + Sync {
    fn on_registered(&self, node: &T);
    fn on_renewed(&self, node: &T);
    fn on_evicted(&self, node: &T);
}

/// Manages leases for a set of nodes
pub struct LeaseManager<T: Clone + Send + Sync + 'static> {
    leases: DashMap<String, Lease<T>>,
    default_duration: Duration,
    observers: RwLock<Vec<Arc<dyn LeaseObserver<T>>>>,
}

impl<T: Clone + Send + Sync + 'static> LeaseManager<T> {
    pub fn new(lease_duration_secs: u64) -> Self {
        Self {
            leases: DashMap::new(),
            default_duration: Duration::from_secs(lease_duration_secs),
            observers: RwLock::new(Vec::new()),
        }
    }

    pub fn register(&self, key: String, node: T) -> bool {
        let lease = Lease::new(node.clone(), self.default_duration);
        let is_new = !self.leases.contains_key(&key);
        self.leases.insert(key, lease);
        
        for obs in self.observers.read().iter() {
            obs.on_registered(&node);
        }
        
        is_new
    }

    pub fn renew(&self, key: &str) -> bool {
        if let Some(mut lease) = self.leases.get_mut(key) {
            lease.renew();
            for obs in self.observers.read().iter() {
                obs.on_renewed(&lease.node);
            }
            true
        } else {
            false
        }
    }

    pub fn get(&self, key: &str) -> Option<T> {
        self.leases.get(key).map(|l| l.node.clone())
    }

    pub fn get_all(&self) -> Vec<T> {
        self.leases.iter().map(|entry| entry.value().node.clone()).collect()
    }

    pub fn get_all_keys(&self) -> Vec<String> {
        self.leases.iter().map(|entry| entry.key().clone()).collect()
    }

    pub fn count(&self) -> usize {
        self.leases.len()
    }

    pub fn remove(&self, key: &str) -> Option<T> {
        self.leases.remove(key).map(|(_, lease)| lease.node)
    }

    /// Evict all expired leases, returning the evicted nodes
    pub fn evict_expired(&self) -> Vec<T> {
        let expired_keys: Vec<String> = self.leases.iter()
            .filter(|entry| entry.value().is_expired())
            .map(|entry| entry.key().clone())
            .collect();

        let mut evicted = Vec::new();
        for key in expired_keys {
            if let Some((_, lease)) = self.leases.remove(&key) {
                warn!("Evicting expired lease for {}", key);
                for obs in self.observers.read().iter() {
                    obs.on_evicted(&lease.node);
                }
                evicted.push(lease.node);
            }
        }
        evicted
    }

    pub fn add_observer(&self, observer: Arc<dyn LeaseObserver<T>>) {
        self.observers.write().push(observer);
    }

    pub fn contains(&self, key: &str) -> bool {
        self.leases.contains_key(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_lease_lifecycle() {
        let manager: LeaseManager<String> = LeaseManager::new(1); // 1 second lease
        
        // Register
        assert!(manager.register("node1".into(), "node1-data".into()));
        assert_eq!(manager.count(), 1);
        
        // Renew
        assert!(manager.renew("node1"));
        assert!(!manager.renew("nonexistent"));
        
        // Get
        assert_eq!(manager.get("node1"), Some("node1-data".into()));
        assert_eq!(manager.get("nonexistent"), None);
    }

    #[test]
    fn test_lease_expiry() {
        let manager: LeaseManager<String> = LeaseManager::new(0); // 0 second lease (expires immediately)
        manager.register("node1".into(), "data".into());
        
        thread::sleep(Duration::from_millis(10));
        
        let evicted = manager.evict_expired();
        assert_eq!(evicted.len(), 1);
        assert_eq!(manager.count(), 0);
    }
}
