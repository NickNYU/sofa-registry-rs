use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use sofa_registry_server_meta::lease::data_server_manager::{DataNode, DataServerManager};
use sofa_registry_server_meta::lease::lease_manager::{Lease, LeaseManager, LeaseObserver};
use sofa_registry_server_meta::lease::session_server_manager::{SessionNode, SessionServerManager};

// ---------------------------------------------------------------------------
// LeaseManager<String> basic tests
// ---------------------------------------------------------------------------

#[test]
fn test_register_returns_true_for_new_node() {
    let mgr: LeaseManager<String> = LeaseManager::new(30);
    assert!(mgr.register("n1".into(), "data1".into()));
}

#[test]
fn test_register_returns_false_for_duplicate_key() {
    let mgr: LeaseManager<String> = LeaseManager::new(30);
    assert!(mgr.register("n1".into(), "data1".into()));
    // Re-register the same key; the old lease is replaced.
    assert!(!mgr.register("n1".into(), "data1-updated".into()));
}

#[test]
fn test_get_returns_registered_value() {
    let mgr: LeaseManager<String> = LeaseManager::new(30);
    mgr.register("k".into(), "v".into());
    assert_eq!(mgr.get("k"), Some("v".to_string()));
}

#[test]
fn test_get_returns_none_for_missing_key() {
    let mgr: LeaseManager<String> = LeaseManager::new(30);
    assert_eq!(mgr.get("missing"), None);
}

#[test]
fn test_count_reflects_registrations() {
    let mgr: LeaseManager<String> = LeaseManager::new(30);
    assert_eq!(mgr.count(), 0);
    mgr.register("a".into(), "1".into());
    assert_eq!(mgr.count(), 1);
    mgr.register("b".into(), "2".into());
    assert_eq!(mgr.count(), 2);
    // Overwrite does not change count
    mgr.register("a".into(), "1-updated".into());
    assert_eq!(mgr.count(), 2);
}

#[test]
fn test_get_all_returns_all_values() {
    let mgr: LeaseManager<String> = LeaseManager::new(30);
    mgr.register("a".into(), "1".into());
    mgr.register("b".into(), "2".into());
    mgr.register("c".into(), "3".into());
    let mut vals = mgr.get_all();
    vals.sort();
    assert_eq!(vals, vec!["1", "2", "3"]);
}

#[test]
fn test_get_all_keys() {
    let mgr: LeaseManager<String> = LeaseManager::new(30);
    mgr.register("x".into(), "data-x".into());
    mgr.register("y".into(), "data-y".into());
    let mut keys = mgr.get_all_keys();
    keys.sort();
    assert_eq!(keys, vec!["x", "y"]);
}

#[test]
fn test_contains() {
    let mgr: LeaseManager<String> = LeaseManager::new(30);
    assert!(!mgr.contains("n1"));
    mgr.register("n1".into(), "d".into());
    assert!(mgr.contains("n1"));
}

#[test]
fn test_remove() {
    let mgr: LeaseManager<String> = LeaseManager::new(30);
    mgr.register("k".into(), "v".into());
    let removed = mgr.remove("k");
    assert_eq!(removed, Some("v".to_string()));
    assert_eq!(mgr.count(), 0);
    assert!(!mgr.contains("k"));
}

#[test]
fn test_remove_nonexistent_returns_none() {
    let mgr: LeaseManager<String> = LeaseManager::new(30);
    assert_eq!(mgr.remove("nope"), None);
}

// ---------------------------------------------------------------------------
// Renew
// ---------------------------------------------------------------------------

#[test]
fn test_renew_existing_returns_true() {
    let mgr: LeaseManager<String> = LeaseManager::new(30);
    mgr.register("n1".into(), "d".into());
    assert!(mgr.renew("n1"));
}

#[test]
fn test_renew_nonexistent_returns_false() {
    let mgr: LeaseManager<String> = LeaseManager::new(30);
    assert!(!mgr.renew("unknown"));
}

// ---------------------------------------------------------------------------
// Eviction
// ---------------------------------------------------------------------------

#[test]
fn test_evict_expired_removes_expired_leases() {
    // Use 0 second lease so it expires immediately
    let mgr: LeaseManager<String> = LeaseManager::new(0);
    mgr.register("n1".into(), "d1".into());
    mgr.register("n2".into(), "d2".into());

    // Wait a bit to ensure expiration
    thread::sleep(Duration::from_millis(10));

    let evicted = mgr.evict_expired();
    assert_eq!(evicted.len(), 2);
    assert_eq!(mgr.count(), 0);
}

#[test]
fn test_evict_does_not_remove_active_leases() {
    let mgr: LeaseManager<String> = LeaseManager::new(60);
    mgr.register("n1".into(), "d1".into());

    let evicted = mgr.evict_expired();
    assert_eq!(evicted.len(), 0);
    assert_eq!(mgr.count(), 1);
}

#[test]
fn test_evict_partial_expiry() {
    // One with immediate expiry, one with 60 seconds
    let short: LeaseManager<String> = LeaseManager::new(0);
    short.register("short".into(), "s".into());

    let long: LeaseManager<String> = LeaseManager::new(60);
    long.register("long".into(), "l".into());

    thread::sleep(Duration::from_millis(10));

    assert_eq!(short.evict_expired().len(), 1);
    assert_eq!(long.evict_expired().len(), 0);
}

#[test]
fn test_renew_prevents_eviction() {
    // Very short lease (1 second)
    let mgr: LeaseManager<String> = LeaseManager::new(1);
    mgr.register("n1".into(), "d".into());

    // Renew before expiry repeatedly to keep alive
    for _ in 0..3 {
        thread::sleep(Duration::from_millis(300));
        assert!(mgr.renew("n1"));
        // Should NOT be expired yet after renew
        assert_eq!(mgr.evict_expired().len(), 0);
    }
    assert_eq!(mgr.count(), 1);
}

#[test]
fn test_evict_expired_returns_correct_nodes() {
    let mgr: LeaseManager<String> = LeaseManager::new(0);
    mgr.register("a".into(), "alpha".into());
    mgr.register("b".into(), "beta".into());

    thread::sleep(Duration::from_millis(10));

    let mut evicted = mgr.evict_expired();
    evicted.sort();
    assert_eq!(evicted, vec!["alpha", "beta"]);
}

// ---------------------------------------------------------------------------
// Lease<T> struct tests
// ---------------------------------------------------------------------------

#[test]
fn test_lease_new_not_expired() {
    let lease: Lease<String> = Lease::new("node".into(), Duration::from_secs(30));
    assert!(!lease.is_expired());
    assert!(lease.remaining() > Duration::ZERO);
}

#[test]
fn test_lease_zero_duration_is_expired() {
    let lease: Lease<String> = Lease::new("node".into(), Duration::ZERO);
    // With zero duration, elapsed > 0 => expired
    thread::sleep(Duration::from_millis(1));
    assert!(lease.is_expired());
    assert_eq!(lease.remaining(), Duration::ZERO);
}

#[test]
fn test_lease_renew_resets_timer() {
    let mut lease: Lease<String> = Lease::new("node".into(), Duration::from_millis(100));
    thread::sleep(Duration::from_millis(60));
    lease.renew();
    // After renew, remaining should be close to the full duration again
    assert!(!lease.is_expired());
    assert!(lease.remaining() > Duration::from_millis(30));
}

// ---------------------------------------------------------------------------
// Observer notifications
// ---------------------------------------------------------------------------

struct CountingObserver {
    registered: AtomicUsize,
    renewed: AtomicUsize,
    evicted: AtomicUsize,
}

impl CountingObserver {
    fn new() -> Self {
        Self {
            registered: AtomicUsize::new(0),
            renewed: AtomicUsize::new(0),
            evicted: AtomicUsize::new(0),
        }
    }
}

impl LeaseObserver<String> for CountingObserver {
    fn on_registered(&self, _node: &String) {
        self.registered.fetch_add(1, Ordering::Relaxed);
    }
    fn on_renewed(&self, _node: &String) {
        self.renewed.fetch_add(1, Ordering::Relaxed);
    }
    fn on_evicted(&self, _node: &String) {
        self.evicted.fetch_add(1, Ordering::Relaxed);
    }
}

#[test]
fn test_observer_on_register() {
    let mgr: LeaseManager<String> = LeaseManager::new(30);
    let obs = Arc::new(CountingObserver::new());
    mgr.add_observer(obs.clone());

    mgr.register("n1".into(), "d1".into());
    mgr.register("n2".into(), "d2".into());

    assert_eq!(obs.registered.load(Ordering::Relaxed), 2);
    assert_eq!(obs.renewed.load(Ordering::Relaxed), 0);
    assert_eq!(obs.evicted.load(Ordering::Relaxed), 0);
}

#[test]
fn test_observer_on_renew() {
    let mgr: LeaseManager<String> = LeaseManager::new(30);
    let obs = Arc::new(CountingObserver::new());
    mgr.add_observer(obs.clone());

    mgr.register("n1".into(), "d1".into());
    mgr.renew("n1");
    mgr.renew("n1");

    assert_eq!(obs.registered.load(Ordering::Relaxed), 1);
    assert_eq!(obs.renewed.load(Ordering::Relaxed), 2);
}

#[test]
fn test_observer_on_evict() {
    let mgr: LeaseManager<String> = LeaseManager::new(0);
    let obs = Arc::new(CountingObserver::new());
    mgr.add_observer(obs.clone());

    mgr.register("n1".into(), "d1".into());
    thread::sleep(Duration::from_millis(10));
    mgr.evict_expired();

    assert_eq!(obs.evicted.load(Ordering::Relaxed), 1);
}

// ---------------------------------------------------------------------------
// Multiple nodes
// ---------------------------------------------------------------------------

#[test]
fn test_multiple_nodes_independent_leases() {
    let mgr: LeaseManager<String> = LeaseManager::new(0);
    for i in 0..10 {
        mgr.register(format!("node-{}", i), format!("data-{}", i));
    }
    assert_eq!(mgr.count(), 10);

    thread::sleep(Duration::from_millis(10));
    let evicted = mgr.evict_expired();
    assert_eq!(evicted.len(), 10);
    assert_eq!(mgr.count(), 0);
}

#[test]
fn test_many_nodes_register_and_retrieve() {
    let mgr: LeaseManager<u64> = LeaseManager::new(60);
    for i in 0..1000 {
        mgr.register(format!("node-{}", i), i);
    }
    assert_eq!(mgr.count(), 1000);

    for i in 0..1000 {
        assert_eq!(mgr.get(&format!("node-{}", i)), Some(i));
    }
}

// ---------------------------------------------------------------------------
// Concurrent access
// ---------------------------------------------------------------------------

#[test]
fn test_concurrent_register_and_renew() {
    let mgr = Arc::new(LeaseManager::<String>::new(60));

    // Pre-register a set of nodes
    for i in 0..10 {
        mgr.register(format!("node-{}", i), format!("data-{}", i));
    }

    let mut handles = vec![];

    // Spawn multiple threads doing renews concurrently
    for _ in 0..4 {
        let mgr = Arc::clone(&mgr);
        handles.push(thread::spawn(move || {
            for _ in 0..100 {
                for i in 0..10 {
                    mgr.renew(&format!("node-{}", i));
                }
            }
        }));
    }

    // Spawn threads doing registrations concurrently
    for t in 0..4 {
        let mgr = Arc::clone(&mgr);
        handles.push(thread::spawn(move || {
            for i in 0..50 {
                mgr.register(format!("thread-{}-node-{}", t, i), format!("d-{}-{}", t, i));
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    // Original 10 + 4 threads * 50 = 210
    assert_eq!(mgr.count(), 210);
}

#[test]
fn test_concurrent_eviction() {
    let mgr = Arc::new(LeaseManager::<String>::new(0));

    for i in 0..100 {
        mgr.register(format!("node-{}", i), format!("data-{}", i));
    }

    thread::sleep(Duration::from_millis(10));

    // Multiple threads trying to evict concurrently
    let mut handles = vec![];
    let total_evicted = Arc::new(AtomicUsize::new(0));

    for _ in 0..4 {
        let mgr = Arc::clone(&mgr);
        let total = Arc::clone(&total_evicted);
        handles.push(thread::spawn(move || {
            let evicted = mgr.evict_expired();
            total.fetch_add(evicted.len(), Ordering::Relaxed);
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    // All 100 nodes should be evicted in total (spread across threads)
    assert_eq!(total_evicted.load(Ordering::Relaxed), 100);
    assert_eq!(mgr.count(), 0);
}

// ---------------------------------------------------------------------------
// DataServerManager
// ---------------------------------------------------------------------------

#[test]
fn test_data_server_manager_register_and_list() {
    let mgr = DataServerManager::new(30);

    let n1 = DataNode::new("10.0.0.1:9621", "dc1", "c1");
    let n2 = DataNode::new("10.0.0.2:9621", "dc1", "c1");

    assert!(mgr.register(n1));
    assert!(mgr.register(n2));
    assert_eq!(mgr.count(), 2);

    let mut addrs = mgr.get_data_server_addresses();
    addrs.sort();
    assert_eq!(addrs, vec!["10.0.0.1:9621", "10.0.0.2:9621"]);
}

#[test]
fn test_data_server_manager_renew() {
    let mgr = DataServerManager::new(30);
    let n1 = DataNode::new("10.0.0.1:9621", "dc1", "c1");
    mgr.register(n1);

    assert!(mgr.renew("10.0.0.1:9621"));
    assert!(!mgr.renew("unknown"));
}

#[test]
fn test_data_server_manager_contains() {
    let mgr = DataServerManager::new(30);
    let n1 = DataNode::new("10.0.0.1:9621", "dc1", "c1");
    mgr.register(n1);

    assert!(mgr.contains("10.0.0.1:9621"));
    assert!(!mgr.contains("10.0.0.2:9621"));
}

#[test]
fn test_data_server_manager_remove() {
    let mgr = DataServerManager::new(30);
    let n1 = DataNode::new("10.0.0.1:9621", "dc1", "c1");
    mgr.register(n1);

    let removed = mgr.remove("10.0.0.1:9621");
    assert!(removed.is_some());
    assert_eq!(removed.unwrap().address, "10.0.0.1:9621");
    assert_eq!(mgr.count(), 0);
}

#[test]
fn test_data_server_manager_evict_expired() {
    let mgr = DataServerManager::new(0);
    mgr.register(DataNode::new("10.0.0.1:9621", "dc1", "c1"));
    mgr.register(DataNode::new("10.0.0.2:9621", "dc1", "c1"));

    thread::sleep(Duration::from_millis(10));

    let evicted = mgr.evict_expired();
    assert_eq!(evicted.len(), 2);
    assert_eq!(mgr.count(), 0);
}

#[test]
fn test_data_server_manager_get_list_returns_node_details() {
    let mgr = DataServerManager::new(30);
    mgr.register(DataNode::new("10.0.0.1:9621", "dc1", "cluster-a"));

    let nodes = mgr.get_data_server_list();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].address, "10.0.0.1:9621");
    assert_eq!(nodes[0].data_center, "dc1");
    assert_eq!(nodes[0].cluster_id, "cluster-a");
}

// ---------------------------------------------------------------------------
// SessionServerManager
// ---------------------------------------------------------------------------

#[test]
fn test_session_server_manager_register_and_list() {
    let mgr = SessionServerManager::new(30);

    let n1 = SessionNode::new("10.0.0.1:9601", "dc1", "c1");
    let n2 = SessionNode::new("10.0.0.2:9601", "dc1", "c1");

    assert!(mgr.register(n1));
    assert!(mgr.register(n2));
    assert_eq!(mgr.count(), 2);

    let mut addrs = mgr.get_session_server_addresses();
    addrs.sort();
    assert_eq!(addrs, vec!["10.0.0.1:9601", "10.0.0.2:9601"]);
}

#[test]
fn test_session_server_manager_renew() {
    let mgr = SessionServerManager::new(30);
    let n1 = SessionNode::new("10.0.0.1:9601", "dc1", "c1");
    mgr.register(n1);

    assert!(mgr.renew("10.0.0.1:9601"));
    assert!(!mgr.renew("unknown-addr"));
}

#[test]
fn test_session_server_manager_contains() {
    let mgr = SessionServerManager::new(30);
    mgr.register(SessionNode::new("10.0.0.1:9601", "dc1", "c1"));

    assert!(mgr.contains("10.0.0.1:9601"));
    assert!(!mgr.contains("nope"));
}

#[test]
fn test_session_server_manager_evict_expired() {
    let mgr = SessionServerManager::new(0);
    mgr.register(SessionNode::new("10.0.0.1:9601", "dc1", "c1"));

    thread::sleep(Duration::from_millis(10));

    let evicted = mgr.evict_expired();
    assert_eq!(evicted.len(), 1);
    assert_eq!(evicted[0].address, "10.0.0.1:9601");
    assert_eq!(mgr.count(), 0);
}

#[test]
fn test_session_server_manager_get_list_returns_node_details() {
    let mgr = SessionServerManager::new(30);
    mgr.register(SessionNode::new("10.0.0.1:9601", "dc1", "cluster-b"));

    let nodes = mgr.get_session_server_list();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].address, "10.0.0.1:9601");
    assert_eq!(nodes[0].data_center, "dc1");
    assert_eq!(nodes[0].cluster_id, "cluster-b");
}
