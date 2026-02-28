use sofa_registry_server_data::lease::SessionLeaseManager;
use std::thread;
use std::time::Duration;

const SESSION_A: &str = "10.0.0.1:9600";
const SESSION_B: &str = "10.0.0.2:9600";
const SESSION_C: &str = "10.0.0.3:9600";
const PID_A: &str = "proc-A";
const PID_B: &str = "proc-B";
const PID_C: &str = "proc-C";

#[test]
fn new_manager_has_no_sessions() {
    let mgr = SessionLeaseManager::new(30);
    assert_eq!(mgr.session_count(), 0);
    assert!(mgr.active_sessions().is_empty());
}

#[test]
fn renew_adds_a_session() {
    let mgr = SessionLeaseManager::new(30);
    mgr.renew(SESSION_A, PID_A);
    assert_eq!(mgr.session_count(), 1);
}

#[test]
fn renew_same_address_replaces_session() {
    let mgr = SessionLeaseManager::new(30);
    mgr.renew(SESSION_A, PID_A);
    mgr.renew(SESSION_A, "proc-A-v2");
    // Still only one session for SESSION_A.
    assert_eq!(mgr.session_count(), 1);
}

#[test]
fn is_active_returns_true_for_valid_lease() {
    let mgr = SessionLeaseManager::new(30);
    mgr.renew(SESSION_A, PID_A);
    assert!(mgr.is_active(SESSION_A));
}

#[test]
fn is_active_returns_false_for_unknown_address() {
    let mgr = SessionLeaseManager::new(30);
    assert!(!mgr.is_active("unknown:1234"));
}

#[test]
fn is_active_returns_false_after_lease_expires() {
    // Lease of 1 second.
    let mgr = SessionLeaseManager::new(1);
    mgr.renew(SESSION_A, PID_A);
    assert!(mgr.is_active(SESSION_A));

    // Wait for the lease to expire.
    thread::sleep(Duration::from_millis(1200));
    assert!(!mgr.is_active(SESSION_A));
}

#[test]
fn renew_extends_lease() {
    // Lease of 1 second.
    let mgr = SessionLeaseManager::new(1);
    mgr.renew(SESSION_A, PID_A);

    // Wait half the lease duration, then renew.
    thread::sleep(Duration::from_millis(600));
    mgr.renew(SESSION_A, PID_A);

    // After another 600ms (total 1200ms from first renew), the session should
    // still be active because the second renew pushed the expiry forward.
    thread::sleep(Duration::from_millis(600));
    assert!(mgr.is_active(SESSION_A));
}

#[test]
fn evict_expired_removes_expired_sessions() {
    let mgr = SessionLeaseManager::new(1);
    mgr.renew(SESSION_A, PID_A);
    mgr.renew(SESSION_B, PID_B);

    // Wait for leases to expire.
    thread::sleep(Duration::from_millis(1200));

    let expired = mgr.evict_expired();
    assert_eq!(expired.len(), 2);
    assert!(expired.contains(&SESSION_A.to_string()));
    assert!(expired.contains(&SESSION_B.to_string()));
    assert_eq!(mgr.session_count(), 0);
}

#[test]
fn evict_expired_keeps_active_sessions() {
    let mgr = SessionLeaseManager::new(1);
    mgr.renew(SESSION_A, PID_A);

    // Wait for A to expire.
    thread::sleep(Duration::from_millis(1200));

    // Renew B just before eviction (B is fresh).
    mgr.renew(SESSION_B, PID_B);

    let expired = mgr.evict_expired();
    assert_eq!(expired.len(), 1);
    assert_eq!(expired[0], SESSION_A);
    assert_eq!(mgr.session_count(), 1);
    assert!(mgr.is_active(SESSION_B));
}

#[test]
fn evict_expired_returns_empty_when_no_expired() {
    let mgr = SessionLeaseManager::new(60);
    mgr.renew(SESSION_A, PID_A);
    let expired = mgr.evict_expired();
    assert!(expired.is_empty());
    assert_eq!(mgr.session_count(), 1);
}

#[test]
fn active_sessions_lists_only_non_expired() {
    let mgr = SessionLeaseManager::new(1);
    mgr.renew(SESSION_A, PID_A);
    mgr.renew(SESSION_B, PID_B);

    // Both should be active initially.
    let active = mgr.active_sessions();
    assert_eq!(active.len(), 2);

    // Wait for expiry.
    thread::sleep(Duration::from_millis(1200));

    // Renew only C.
    mgr.renew(SESSION_C, PID_C);

    let active = mgr.active_sessions();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0], SESSION_C);
}

#[test]
fn remove_deletes_specific_session() {
    let mgr = SessionLeaseManager::new(60);
    mgr.renew(SESSION_A, PID_A);
    mgr.renew(SESSION_B, PID_B);
    assert_eq!(mgr.session_count(), 2);

    mgr.remove(SESSION_A);
    assert_eq!(mgr.session_count(), 1);
    assert!(!mgr.is_active(SESSION_A));
    assert!(mgr.is_active(SESSION_B));
}

#[test]
fn remove_nonexistent_is_noop() {
    let mgr = SessionLeaseManager::new(60);
    mgr.remove("does-not-exist:1234");
    assert_eq!(mgr.session_count(), 0);
}

#[test]
fn session_count_reflects_all_tracked_sessions() {
    let mgr = SessionLeaseManager::new(60);
    assert_eq!(mgr.session_count(), 0);

    mgr.renew(SESSION_A, PID_A);
    assert_eq!(mgr.session_count(), 1);

    mgr.renew(SESSION_B, PID_B);
    assert_eq!(mgr.session_count(), 2);

    mgr.renew(SESSION_C, PID_C);
    assert_eq!(mgr.session_count(), 3);

    mgr.remove(SESSION_B);
    assert_eq!(mgr.session_count(), 2);
}

#[test]
fn multiple_renews_to_different_addresses() {
    let mgr = SessionLeaseManager::new(60);

    for i in 0..50 {
        let addr = format!("10.0.0.{}:9600", i);
        let pid = format!("proc-{}", i);
        mgr.renew(&addr, &pid);
    }

    assert_eq!(mgr.session_count(), 50);
    assert_eq!(mgr.active_sessions().len(), 50);
}
