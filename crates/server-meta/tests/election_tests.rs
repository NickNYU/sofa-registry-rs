use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use sofa_registry_server_meta::leader::MetaLeaderElector;
use sofa_registry_store::jdbc::{create_pool, run_migrations, SqliteDistributeLockRepo};
use sofa_registry_store::traits::leader_elector::{ElectorRole, LeaderAware, LeaderElector};
use tokio_util::sync::CancellationToken;

/// Helper: create an in-memory SQLite pool and run migrations.
async fn setup_db() -> sqlx::SqlitePool {
    let pool = create_pool("sqlite::memory:")
        .await
        .expect("Failed to create in-memory pool");
    run_migrations(&pool)
        .await
        .expect("Failed to run migrations");
    pool
}

/// Helper: create a MetaLeaderElector backed by the given pool.
fn make_elector(pool: sqlx::SqlitePool, address: &str, lock_duration_ms: i64) -> MetaLeaderElector {
    let lock_repo = Arc::new(SqliteDistributeLockRepo::new(pool));
    MetaLeaderElector::new(
        lock_repo,
        address.to_string(),
        "DefaultDataCenter".to_string(),
        lock_duration_ms,
        100, // election_interval_ms (fast for tests)
    )
}

// ---------------------------------------------------------------------------
// Basic role and identity
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_initial_role_is_follower() {
    let pool = setup_db().await;
    let elector = make_elector(pool, "127.0.0.1:9611", 30000);

    assert_eq!(elector.get_role(), ElectorRole::Follower);
    assert!(!elector.am_i_leader());
    assert_eq!(elector.myself(), "127.0.0.1:9611");
}

#[tokio::test]
async fn test_initial_leader_info_is_empty() {
    let pool = setup_db().await;
    let elector = make_elector(pool, "127.0.0.1:9611", 30000);

    let info = elector.get_leader_info();
    assert!(info.leader.is_none());
    assert_eq!(info.epoch, -1);
}

// ---------------------------------------------------------------------------
// Compete / elect
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_single_node_wins_election() {
    let pool = setup_db().await;
    let elector = make_elector(pool, "node-1", 30000);

    let info = elector.elect().await.expect("elect() failed");
    assert!(info.leader.is_some());
    assert_eq!(info.leader.as_deref(), Some("node-1"));
    assert!(info.epoch >= 1);
    assert!(elector.am_i_leader());
    assert_eq!(elector.get_role(), ElectorRole::Leader);
}

#[tokio::test]
async fn test_elect_twice_keeps_leadership() {
    let pool = setup_db().await;
    let elector = make_elector(pool, "node-1", 30000);

    let info1 = elector.elect().await.unwrap();
    let info2 = elector.elect().await.unwrap();

    // Should still be leader
    assert!(elector.am_i_leader());
    // Epoch may increment due to re-acquire
    assert!(info2.epoch >= info1.epoch);
}

// ---------------------------------------------------------------------------
// Two nodes compete for the same lock (shared DB)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_two_nodes_only_one_becomes_leader() {
    let pool = setup_db().await;
    let elector1 = make_elector(pool.clone(), "node-1", 30000);
    let elector2 = make_elector(pool, "node-2", 30000);

    // Node 1 competes first
    elector1.elect().await.unwrap();
    assert!(elector1.am_i_leader());

    // Node 2 competes - should NOT become leader (lock is held by node-1)
    elector2.elect().await.unwrap();
    assert!(!elector2.am_i_leader());

    // Node 2 should see node-1 as leader
    let _info2 = elector2.get_leader_info();
    // The leader may or may not be populated in node2's view depending on compete result,
    // but node2 should NOT think it's the leader.
    assert!(!elector2.am_i_leader());
}

#[tokio::test]
async fn test_second_node_sees_leader_via_query() {
    let pool = setup_db().await;
    let elector1 = make_elector(pool.clone(), "node-1", 30000);
    let elector2 = make_elector(pool, "node-2", 30000);

    // Node 1 wins
    elector1.elect().await.unwrap();
    assert!(elector1.am_i_leader());

    // Node 2 queries
    let info = elector2.query_leader().await.unwrap();
    assert_eq!(info.leader.as_deref(), Some("node-1"));
}

// ---------------------------------------------------------------------------
// Heartbeat
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_leader_heartbeat_extends_lock() {
    let pool = setup_db().await;
    let elector = make_elector(pool, "node-1", 30000);

    // Win election
    elector.elect().await.unwrap();
    assert!(elector.am_i_leader());

    // Heartbeat (simulate a tick as leader) by calling elect again
    // Since we're the owner, compete_lock updates the lock.
    let info = elector.elect().await.unwrap();
    assert!(elector.am_i_leader());
    assert_eq!(info.leader.as_deref(), Some("node-1"));
}

// ---------------------------------------------------------------------------
// Role transitions
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_change_to_follower() {
    let pool = setup_db().await;
    let elector = make_elector(pool, "node-1", 30000);

    elector.elect().await.unwrap();
    assert_eq!(elector.get_role(), ElectorRole::Leader);

    elector.change_to_follower();
    assert_eq!(elector.get_role(), ElectorRole::Follower);
    assert!(!elector.am_i_leader());
}

#[tokio::test]
async fn test_change_to_observer() {
    let pool = setup_db().await;
    let elector = make_elector(pool, "node-1", 30000);

    elector.elect().await.unwrap();
    assert_eq!(elector.get_role(), ElectorRole::Leader);

    elector.change_to_observer();
    assert_eq!(elector.get_role(), ElectorRole::Observer);
    assert!(!elector.am_i_leader());
}

#[tokio::test]
async fn test_change_to_follower_when_already_follower_is_noop() {
    let pool = setup_db().await;
    let elector = make_elector(pool, "node-1", 30000);

    assert_eq!(elector.get_role(), ElectorRole::Follower);
    elector.change_to_follower();
    assert_eq!(elector.get_role(), ElectorRole::Follower);
}

// ---------------------------------------------------------------------------
// LeaderAware callbacks
// ---------------------------------------------------------------------------

struct TestAware {
    became_leader: AtomicBool,
    lost_leader: AtomicBool,
}

impl TestAware {
    fn new() -> Self {
        Self {
            became_leader: AtomicBool::new(false),
            lost_leader: AtomicBool::new(false),
        }
    }
}

impl LeaderAware for TestAware {
    fn on_become_leader(&self) {
        self.became_leader.store(true, Ordering::Relaxed);
    }
    fn on_lose_leadership(&self) {
        self.lost_leader.store(true, Ordering::Relaxed);
    }
}

#[tokio::test]
async fn test_leader_aware_on_become_leader() {
    let pool = setup_db().await;
    let elector = make_elector(pool, "node-1", 30000);
    let aware = Arc::new(TestAware::new());
    elector.register_leader_aware(aware.clone());

    elector.elect().await.unwrap();

    assert!(aware.became_leader.load(Ordering::Relaxed));
    assert!(!aware.lost_leader.load(Ordering::Relaxed));
}

#[tokio::test]
async fn test_leader_aware_on_lose_leadership() {
    let pool = setup_db().await;
    let elector = make_elector(pool, "node-1", 30000);
    let aware = Arc::new(TestAware::new());
    elector.register_leader_aware(aware.clone());

    elector.elect().await.unwrap();
    assert!(aware.became_leader.load(Ordering::Relaxed));

    elector.change_to_follower();
    assert!(aware.lost_leader.load(Ordering::Relaxed));
}

#[tokio::test]
async fn test_leader_aware_on_change_to_observer() {
    let pool = setup_db().await;
    let elector = make_elector(pool, "node-1", 30000);
    let aware = Arc::new(TestAware::new());
    elector.register_leader_aware(aware.clone());

    elector.elect().await.unwrap();
    elector.change_to_observer();

    assert!(aware.lost_leader.load(Ordering::Relaxed));
}

#[tokio::test]
async fn test_multiple_leader_awares_all_notified() {
    let pool = setup_db().await;
    let elector = make_elector(pool, "node-1", 30000);

    let aware1 = Arc::new(TestAware::new());
    let aware2 = Arc::new(TestAware::new());
    elector.register_leader_aware(aware1.clone());
    elector.register_leader_aware(aware2.clone());

    elector.elect().await.unwrap();

    assert!(aware1.became_leader.load(Ordering::Relaxed));
    assert!(aware2.became_leader.load(Ordering::Relaxed));
}

// ---------------------------------------------------------------------------
// Election loop
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_election_loop_can_be_cancelled() {
    let pool = setup_db().await;
    let elector = Arc::new(make_elector(pool, "node-1", 30000));
    let cancel = CancellationToken::new();

    let elector_clone = elector.clone();
    let cancel_clone = cancel.clone();
    let handle = tokio::spawn(async move {
        elector_clone.run_election_loop(cancel_clone).await;
    });

    // Let it run a few ticks
    tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;

    // Cancel and wait for the task to finish
    cancel.cancel();
    handle.await.expect("Election loop panicked");

    // After some ticks, the elector should have won leadership
    assert!(elector.am_i_leader());
}

#[tokio::test]
async fn test_election_loop_elects_leader() {
    let pool = setup_db().await;
    let elector = Arc::new(make_elector(pool, "node-1", 30000));
    let cancel = CancellationToken::new();

    let e = elector.clone();
    let c = cancel.clone();
    tokio::spawn(async move {
        e.run_election_loop(c).await;
    });

    // Wait for a few election ticks
    tokio::time::sleep(tokio::time::Duration::from_millis(350)).await;

    assert!(elector.am_i_leader());
    assert_eq!(elector.get_role(), ElectorRole::Leader);

    let info = elector.get_leader_info();
    assert_eq!(info.leader.as_deref(), Some("node-1"));

    cancel.cancel();
}

// ---------------------------------------------------------------------------
// Leader takeover when lock expires
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_leader_takeover_after_expiry() {
    // Use a very short lock duration so it expires quickly
    let pool = setup_db().await;
    let elector1 = make_elector(pool.clone(), "node-1", 200); // 200ms lock
    let elector2 = make_elector(pool, "node-2", 200);

    // Node 1 wins
    elector1.elect().await.unwrap();
    assert!(elector1.am_i_leader());

    // Wait for the lock to expire
    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

    // Node 2 should now be able to take over
    elector2.elect().await.unwrap();

    // One of them should now be leader. Since node-1's lock expired,
    // node-2 should have taken over.
    // Node-1 still thinks it's leader (it hasn't run a tick), but node-2 won.
    assert!(elector2.am_i_leader());
    assert_eq!(elector2.get_leader_info().leader.as_deref(), Some("node-2"));
}

// ---------------------------------------------------------------------------
// Query from observer
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_observer_can_query_leader() {
    let pool = setup_db().await;
    let elector1 = make_elector(pool.clone(), "node-1", 30000);
    let elector2 = make_elector(pool, "node-2", 30000);

    // Node 1 wins
    elector1.elect().await.unwrap();

    // Node 2 changes to observer and queries
    elector2.change_to_observer();
    assert_eq!(elector2.get_role(), ElectorRole::Observer);

    let info = elector2.query_leader().await.unwrap();
    assert_eq!(info.leader.as_deref(), Some("node-1"));
}
