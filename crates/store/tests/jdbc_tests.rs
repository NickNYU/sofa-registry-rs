use chrono::Utc;
use sofa_registry_store::jdbc::{
    create_pool, run_migrations, SqliteAppRevisionRepo, SqliteClientManagerRepo,
    SqliteDistributeLockRepo, SqliteInterfaceAppsRepo, SqliteProvideDataRepo,
};
use sofa_registry_store::traits::{
    AppRevision, AppRevisionRepository, ClientManagerAddressRepository, DistributeLockRepository,
    InterfaceAppsRepository, PersistenceData, ProvideDataRepository,
};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Helper: create an in-memory SQLite pool with migrations applied
// ---------------------------------------------------------------------------

async fn setup_pool() -> sqlx::SqlitePool {
    let pool = create_pool("sqlite::memory:")
        .await
        .expect("pool creation failed");
    run_migrations(&pool).await.expect("migrations failed");
    pool
}

// ===========================================================================
// DistributeLockRepository tests
// ===========================================================================

#[tokio::test]
async fn distribute_lock_compete_and_query() {
    let pool = setup_pool().await;
    let repo = SqliteDistributeLockRepo::new(pool);

    let lock = repo
        .compete_lock("leader-election", "dc1", "node-1", 30_000)
        .await
        .expect("compete_lock should succeed");
    assert!(lock.is_some(), "should acquire the lock");

    let lock = lock.unwrap();
    assert_eq!(lock.lock_name, "leader-election");
    assert_eq!(lock.data_center, "dc1");
    assert_eq!(lock.owner, "node-1");
    assert_eq!(lock.term, 1);
    assert_eq!(lock.duration, 30_000);

    // Query the lock
    let queried = repo
        .query_lock("leader-election", "dc1")
        .await
        .expect("query_lock should succeed")
        .expect("lock should exist");
    assert_eq!(queried.owner, "node-1");
    assert_eq!(queried.term, 1);
}

#[tokio::test]
async fn distribute_lock_query_nonexistent() {
    let pool = setup_pool().await;
    let repo = SqliteDistributeLockRepo::new(pool);

    let lock = repo
        .query_lock("no-lock", "dc1")
        .await
        .expect("query should succeed");
    assert!(lock.is_none());
}

#[tokio::test]
async fn distribute_lock_same_owner_reacquires() {
    let pool = setup_pool().await;
    let repo = SqliteDistributeLockRepo::new(pool);

    // First acquire
    let lock1 = repo
        .compete_lock("lock-a", "dc1", "node-1", 30_000)
        .await
        .expect("first compete should succeed")
        .expect("should acquire lock");
    assert_eq!(lock1.term, 1);

    // Same owner competes again - should succeed with incremented term
    let lock2 = repo
        .compete_lock("lock-a", "dc1", "node-1", 30_000)
        .await
        .expect("second compete should succeed")
        .expect("same owner should re-acquire");
    assert_eq!(lock2.owner, "node-1");
    assert_eq!(lock2.term, 2);
}

#[tokio::test]
async fn distribute_lock_different_owner_cannot_take_active_lock() {
    let pool = setup_pool().await;
    let repo = SqliteDistributeLockRepo::new(pool);

    // node-1 acquires with a long duration
    let _lock1 = repo
        .compete_lock("lock-a", "dc1", "node-1", 60_000_000)
        .await
        .expect("should succeed")
        .expect("should acquire");

    // node-2 tries to compete -- should fail because lock is active (not expired)
    let lock2 = repo
        .compete_lock("lock-a", "dc1", "node-2", 30_000)
        .await
        .expect("should succeed");
    assert!(
        lock2.is_none(),
        "different owner should not take an active lock"
    );
}

#[tokio::test]
async fn distribute_lock_owner_heartbeat() {
    let pool = setup_pool().await;
    let repo = SqliteDistributeLockRepo::new(pool);

    // Acquire
    repo.compete_lock("lock-hb", "dc1", "node-1", 30_000)
        .await
        .unwrap()
        .unwrap();

    // Heartbeat by owner
    let result = repo
        .owner_heartbeat("lock-hb", "dc1", "node-1", 60_000)
        .await
        .expect("heartbeat should succeed");
    assert!(result, "heartbeat should return true for owner");

    // Verify duration was updated
    let lock = repo.query_lock("lock-hb", "dc1").await.unwrap().unwrap();
    assert_eq!(lock.duration, 60_000);
}

#[tokio::test]
async fn distribute_lock_heartbeat_wrong_owner_fails() {
    let pool = setup_pool().await;
    let repo = SqliteDistributeLockRepo::new(pool);

    repo.compete_lock("lock-hb2", "dc1", "node-1", 30_000)
        .await
        .unwrap()
        .unwrap();

    // Heartbeat by a different owner
    let result = repo
        .owner_heartbeat("lock-hb2", "dc1", "node-2", 60_000)
        .await
        .expect("should not error");
    assert!(!result, "heartbeat by wrong owner should return false");
}

#[tokio::test]
async fn distribute_lock_heartbeat_nonexistent() {
    let pool = setup_pool().await;
    let repo = SqliteDistributeLockRepo::new(pool);

    let result = repo
        .owner_heartbeat("no-lock", "dc1", "node-1", 30_000)
        .await
        .expect("should not error");
    assert!(!result, "heartbeat on nonexistent lock should return false");
}

#[tokio::test]
async fn distribute_lock_separate_data_centers() {
    let pool = setup_pool().await;
    let repo = SqliteDistributeLockRepo::new(pool);

    // Same lock name, different data centers
    let lock1 = repo
        .compete_lock("leader", "dc1", "node-1", 30_000)
        .await
        .unwrap()
        .unwrap();
    let lock2 = repo
        .compete_lock("leader", "dc2", "node-2", 30_000)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(lock1.owner, "node-1");
    assert_eq!(lock2.owner, "node-2");
}

// ===========================================================================
// AppRevisionRepository tests
// ===========================================================================

fn make_app_revision(dc: &str, rev: &str, app: &str) -> AppRevision {
    AppRevision {
        data_center: dc.to_string(),
        revision: rev.to_string(),
        app_name: app.to_string(),
        base_params: {
            let mut m = HashMap::new();
            m.insert("key1".to_string(), "val1".to_string());
            m
        },
        service_params: {
            let mut outer = HashMap::new();
            let mut inner = HashMap::new();
            inner.insert("sk".to_string(), "sv".to_string());
            outer.insert("svc-a".to_string(), inner);
            outer
        },
        deleted: false,
        gmt_create: Utc::now(),
        gmt_modified: Utc::now(),
    }
}

#[tokio::test]
async fn app_revision_register_and_query() {
    let pool = setup_pool().await;
    let repo = SqliteAppRevisionRepo::new(pool);

    let rev = make_app_revision("dc1", "rev-001", "my-app");
    repo.register(rev).await.expect("register should succeed");

    let queried = repo
        .query_revision("rev-001")
        .await
        .expect("query should succeed")
        .expect("revision should exist");
    assert_eq!(queried.data_center, "dc1");
    assert_eq!(queried.app_name, "my-app");
    assert_eq!(queried.base_params.get("key1").unwrap(), "val1");
    assert!(!queried.deleted);
    assert!(queried.service_params.contains_key("svc-a"));
}

#[tokio::test]
async fn app_revision_query_nonexistent() {
    let pool = setup_pool().await;
    let repo = SqliteAppRevisionRepo::new(pool);

    let result = repo.query_revision("no-rev").await.expect("should succeed");
    assert!(result.is_none());
}

#[tokio::test]
async fn app_revision_register_upsert() {
    let pool = setup_pool().await;
    let repo = SqliteAppRevisionRepo::new(pool);

    let rev1 = make_app_revision("dc1", "rev-001", "old-app");
    repo.register(rev1).await.unwrap();

    // Register again with same (dc, revision) but different app_name
    let rev2 = make_app_revision("dc1", "rev-001", "new-app");
    repo.register(rev2).await.unwrap();

    let queried = repo.query_revision("rev-001").await.unwrap().unwrap();
    assert_eq!(queried.app_name, "new-app", "upsert should update app_name");
}

#[tokio::test]
async fn app_revision_heartbeat() {
    let pool = setup_pool().await;
    let repo = SqliteAppRevisionRepo::new(pool);

    let rev = make_app_revision("dc1", "rev-hb", "my-app");
    repo.register(rev).await.unwrap();

    let result = repo
        .heartbeat("rev-hb")
        .await
        .expect("heartbeat should succeed");
    assert!(result, "heartbeat should return true for existing revision");
}

#[tokio::test]
async fn app_revision_heartbeat_nonexistent() {
    let pool = setup_pool().await;
    let repo = SqliteAppRevisionRepo::new(pool);

    let result = repo.heartbeat("no-rev").await.expect("should succeed");
    assert!(!result, "heartbeat on nonexistent should return false");
}

#[tokio::test]
async fn app_revision_heartbeat_deleted_returns_false() {
    let pool = setup_pool().await;
    let repo = SqliteAppRevisionRepo::new(pool);

    let mut rev = make_app_revision("dc1", "rev-del", "my-app");
    rev.deleted = true;
    repo.register(rev).await.unwrap();

    let result = repo.heartbeat("rev-del").await.unwrap();
    assert!(!result, "heartbeat on deleted revision should return false");
}

#[tokio::test]
async fn app_revision_get_expired() {
    let pool = setup_pool().await;
    let repo = SqliteAppRevisionRepo::new(pool);

    // Register revisions (their gmt_modified will be approximately now)
    let rev1 = make_app_revision("dc1", "rev-exp-1", "app-a");
    let rev2 = make_app_revision("dc1", "rev-exp-2", "app-b");
    repo.register(rev1).await.unwrap();
    repo.register(rev2).await.unwrap();

    // Querying expired before a time in the future should return them
    let future = Utc::now() + chrono::Duration::hours(1);
    let expired = repo.get_expired(future, 100).await.expect("should succeed");
    assert_eq!(expired.len(), 2);

    // Querying expired before a time in the past should return none
    let past = Utc::now() - chrono::Duration::hours(1);
    let expired = repo.get_expired(past, 100).await.expect("should succeed");
    assert_eq!(expired.len(), 0);
}

#[tokio::test]
async fn app_revision_get_expired_respects_limit() {
    let pool = setup_pool().await;
    let repo = SqliteAppRevisionRepo::new(pool);

    for i in 0..5 {
        let rev = make_app_revision("dc1", &format!("rev-lim-{}", i), "app");
        repo.register(rev).await.unwrap();
    }

    let future = Utc::now() + chrono::Duration::hours(1);
    let expired = repo.get_expired(future, 2).await.unwrap();
    assert_eq!(expired.len(), 2, "limit should be respected");
}

#[tokio::test]
async fn app_revision_get_expired_excludes_deleted() {
    let pool = setup_pool().await;
    let repo = SqliteAppRevisionRepo::new(pool);

    let rev1 = make_app_revision("dc1", "rev-active", "app");
    let mut rev2 = make_app_revision("dc1", "rev-deleted", "app");
    rev2.deleted = true;
    repo.register(rev1).await.unwrap();
    repo.register(rev2).await.unwrap();

    let future = Utc::now() + chrono::Duration::hours(1);
    let expired = repo.get_expired(future, 100).await.unwrap();
    assert_eq!(expired.len(), 1, "deleted revisions should be excluded");
    assert_eq!(expired[0].revision, "rev-active");
}

#[tokio::test]
async fn app_revision_clean_deleted() {
    let pool = setup_pool().await;
    let repo = SqliteAppRevisionRepo::new(pool);

    let mut rev1 = make_app_revision("dc1", "rev-d1", "app");
    rev1.deleted = true;
    let mut rev2 = make_app_revision("dc1", "rev-d2", "app");
    rev2.deleted = true;
    let rev3 = make_app_revision("dc1", "rev-active", "app");

    repo.register(rev1).await.unwrap();
    repo.register(rev2).await.unwrap();
    repo.register(rev3).await.unwrap();

    let future = Utc::now() + chrono::Duration::hours(1);
    let cleaned = repo
        .clean_deleted(future, 100)
        .await
        .expect("should succeed");
    assert_eq!(cleaned, 2, "two deleted revisions should be cleaned");

    // Active one should still exist
    let active = repo.query_revision("rev-active").await.unwrap();
    assert!(active.is_some());

    // Deleted ones should be gone
    assert!(repo.query_revision("rev-d1").await.unwrap().is_none());
    assert!(repo.query_revision("rev-d2").await.unwrap().is_none());
}

#[tokio::test]
async fn app_revision_clean_deleted_respects_limit() {
    let pool = setup_pool().await;
    let repo = SqliteAppRevisionRepo::new(pool);

    for i in 0..5 {
        let mut rev = make_app_revision("dc1", &format!("rev-cd-{}", i), "app");
        rev.deleted = true;
        repo.register(rev).await.unwrap();
    }

    let future = Utc::now() + chrono::Duration::hours(1);
    let cleaned = repo.clean_deleted(future, 2).await.unwrap();
    assert_eq!(cleaned, 2, "limit should be respected");
}

// ===========================================================================
// ProvideDataRepository tests
// ===========================================================================

#[tokio::test]
async fn provide_data_put_and_get() {
    let pool = setup_pool().await;
    let repo = SqliteProvideDataRepo::new(pool);

    let data = PersistenceData {
        data_center: "dc1".to_string(),
        data_key: "config.key1".to_string(),
        data_value: "value-1".to_string(),
        version: 1,
    };
    let result = repo.put(data).await.expect("put should succeed");
    assert!(result);

    let queried = repo
        .get("dc1", "config.key1")
        .await
        .expect("get should succeed")
        .expect("data should exist");
    assert_eq!(queried.data_center, "dc1");
    assert_eq!(queried.data_key, "config.key1");
    assert_eq!(queried.data_value, "value-1");
    assert_eq!(queried.version, 1);
}

#[tokio::test]
async fn provide_data_get_nonexistent() {
    let pool = setup_pool().await;
    let repo = SqliteProvideDataRepo::new(pool);

    let result = repo.get("dc1", "no-key").await.expect("should succeed");
    assert!(result.is_none());
}

#[tokio::test]
async fn provide_data_put_upsert() {
    let pool = setup_pool().await;
    let repo = SqliteProvideDataRepo::new(pool);

    let data1 = PersistenceData {
        data_center: "dc1".to_string(),
        data_key: "key".to_string(),
        data_value: "old".to_string(),
        version: 1,
    };
    repo.put(data1).await.unwrap();

    let data2 = PersistenceData {
        data_center: "dc1".to_string(),
        data_key: "key".to_string(),
        data_value: "new".to_string(),
        version: 2,
    };
    repo.put(data2).await.unwrap();

    let queried = repo.get("dc1", "key").await.unwrap().unwrap();
    assert_eq!(queried.data_value, "new");
    assert_eq!(queried.version, 2);
}

#[tokio::test]
async fn provide_data_remove() {
    let pool = setup_pool().await;
    let repo = SqliteProvideDataRepo::new(pool);

    let data = PersistenceData {
        data_center: "dc1".to_string(),
        data_key: "key-rm".to_string(),
        data_value: "val".to_string(),
        version: 1,
    };
    repo.put(data).await.unwrap();

    let removed = repo.remove("dc1", "key-rm").await.expect("should succeed");
    assert!(removed);

    let queried = repo.get("dc1", "key-rm").await.unwrap();
    assert!(queried.is_none(), "removed data should be gone");
}

#[tokio::test]
async fn provide_data_remove_nonexistent() {
    let pool = setup_pool().await;
    let repo = SqliteProvideDataRepo::new(pool);

    let removed = repo.remove("dc1", "no-key").await.unwrap();
    assert!(!removed, "removing nonexistent should return false");
}

#[tokio::test]
async fn provide_data_get_all() {
    let pool = setup_pool().await;
    let repo = SqliteProvideDataRepo::new(pool);

    for i in 0..3 {
        let data = PersistenceData {
            data_center: "dc1".to_string(),
            data_key: format!("key-{}", i),
            data_value: format!("val-{}", i),
            version: i as i64,
        };
        repo.put(data).await.unwrap();
    }

    // Also insert in a different DC
    let data_dc2 = PersistenceData {
        data_center: "dc2".to_string(),
        data_key: "key-other".to_string(),
        data_value: "val-other".to_string(),
        version: 0,
    };
    repo.put(data_dc2).await.unwrap();

    let all_dc1 = repo.get_all("dc1").await.expect("should succeed");
    assert_eq!(all_dc1.len(), 3);

    let all_dc2 = repo.get_all("dc2").await.unwrap();
    assert_eq!(all_dc2.len(), 1);

    let all_dc3 = repo.get_all("dc3").await.unwrap();
    assert!(all_dc3.is_empty());
}

// ===========================================================================
// InterfaceAppsRepository tests
// ===========================================================================

#[tokio::test]
async fn interface_apps_register_and_get() {
    let pool = setup_pool().await;
    let repo = SqliteInterfaceAppsRepo::new(pool);

    repo.register("dc1", "app-a", "com.example.ServiceA")
        .await
        .expect("register should succeed");
    repo.register("dc1", "app-b", "com.example.ServiceA")
        .await
        .expect("register should succeed");

    let mut apps = repo
        .get_app_names("dc1", "com.example.ServiceA")
        .await
        .expect("get should succeed");
    apps.sort();
    assert_eq!(apps, vec!["app-a", "app-b"]);
}

#[tokio::test]
async fn interface_apps_get_nonexistent() {
    let pool = setup_pool().await;
    let repo = SqliteInterfaceAppsRepo::new(pool);

    let apps = repo
        .get_app_names("dc1", "no-interface")
        .await
        .expect("should succeed");
    assert!(apps.is_empty());
}

#[tokio::test]
async fn interface_apps_register_idempotent() {
    let pool = setup_pool().await;
    let repo = SqliteInterfaceAppsRepo::new(pool);

    // Register the same mapping twice
    repo.register("dc1", "app-a", "com.example.ServiceA")
        .await
        .unwrap();
    repo.register("dc1", "app-a", "com.example.ServiceA")
        .await
        .unwrap();

    let apps = repo
        .get_app_names("dc1", "com.example.ServiceA")
        .await
        .unwrap();
    assert_eq!(
        apps.len(),
        1,
        "duplicate register should not create duplicate"
    );
}

#[tokio::test]
async fn interface_apps_different_interfaces() {
    let pool = setup_pool().await;
    let repo = SqliteInterfaceAppsRepo::new(pool);

    repo.register("dc1", "app-a", "svc-1").await.unwrap();
    repo.register("dc1", "app-a", "svc-2").await.unwrap();
    repo.register("dc1", "app-b", "svc-1").await.unwrap();

    let svc1_apps = repo.get_app_names("dc1", "svc-1").await.unwrap();
    assert_eq!(svc1_apps.len(), 2);

    let svc2_apps = repo.get_app_names("dc1", "svc-2").await.unwrap();
    assert_eq!(svc2_apps.len(), 1);
}

#[tokio::test]
async fn interface_apps_separate_data_centers() {
    let pool = setup_pool().await;
    let repo = SqliteInterfaceAppsRepo::new(pool);

    repo.register("dc1", "app-a", "svc-1").await.unwrap();
    repo.register("dc2", "app-b", "svc-1").await.unwrap();

    let dc1_apps = repo.get_app_names("dc1", "svc-1").await.unwrap();
    assert_eq!(dc1_apps, vec!["app-a"]);

    let dc2_apps = repo.get_app_names("dc2", "svc-1").await.unwrap();
    assert_eq!(dc2_apps, vec!["app-b"]);
}

// ===========================================================================
// ClientManagerAddressRepository tests
// ===========================================================================

#[tokio::test]
async fn client_manager_client_off_and_get() {
    let pool = setup_pool().await;
    let repo = SqliteClientManagerRepo::new(pool);

    let result = repo
        .client_off("dc1", "10.0.0.1:9600")
        .await
        .expect("client_off should succeed");
    assert!(result);

    let addrs = repo
        .get_client_off_addresses("dc1")
        .await
        .expect("get should succeed");
    assert_eq!(addrs.len(), 1);
    assert_eq!(addrs[0].address, "10.0.0.1:9600");
    assert_eq!(addrs[0].operation, "CLIENT_OFF");
    assert_eq!(addrs[0].data_center, "dc1");
}

#[tokio::test]
async fn client_manager_get_empty() {
    let pool = setup_pool().await;
    let repo = SqliteClientManagerRepo::new(pool);

    let addrs = repo.get_client_off_addresses("dc1").await.unwrap();
    assert!(addrs.is_empty());
}

#[tokio::test]
async fn client_manager_client_on_removes() {
    let pool = setup_pool().await;
    let repo = SqliteClientManagerRepo::new(pool);

    repo.client_off("dc1", "10.0.0.1:9600").await.unwrap();
    repo.client_off("dc1", "10.0.0.2:9600").await.unwrap();

    let result = repo
        .client_on("dc1", "10.0.0.1:9600")
        .await
        .expect("client_on should succeed");
    assert!(result);

    let addrs = repo.get_client_off_addresses("dc1").await.unwrap();
    assert_eq!(addrs.len(), 1);
    assert_eq!(addrs[0].address, "10.0.0.2:9600");
}

#[tokio::test]
async fn client_manager_client_on_nonexistent() {
    let pool = setup_pool().await;
    let repo = SqliteClientManagerRepo::new(pool);

    let result = repo.client_on("dc1", "no-addr").await.unwrap();
    assert!(
        !result,
        "client_on for nonexistent address should return false"
    );
}

#[tokio::test]
async fn client_manager_client_off_idempotent() {
    let pool = setup_pool().await;
    let repo = SqliteClientManagerRepo::new(pool);

    repo.client_off("dc1", "10.0.0.1:9600").await.unwrap();
    repo.client_off("dc1", "10.0.0.1:9600").await.unwrap();

    let addrs = repo.get_client_off_addresses("dc1").await.unwrap();
    assert_eq!(
        addrs.len(),
        1,
        "duplicate client_off should not create duplicate"
    );
}

#[tokio::test]
async fn client_manager_separate_data_centers() {
    let pool = setup_pool().await;
    let repo = SqliteClientManagerRepo::new(pool);

    repo.client_off("dc1", "10.0.0.1:9600").await.unwrap();
    repo.client_off("dc2", "10.0.0.2:9600").await.unwrap();

    let dc1 = repo.get_client_off_addresses("dc1").await.unwrap();
    assert_eq!(dc1.len(), 1);
    assert_eq!(dc1[0].address, "10.0.0.1:9600");

    let dc2 = repo.get_client_off_addresses("dc2").await.unwrap();
    assert_eq!(dc2.len(), 1);
    assert_eq!(dc2[0].address, "10.0.0.2:9600");
}

#[tokio::test]
async fn client_manager_multiple_addresses() {
    let pool = setup_pool().await;
    let repo = SqliteClientManagerRepo::new(pool);

    for i in 1..=5 {
        repo.client_off("dc1", &format!("10.0.0.{}:9600", i))
            .await
            .unwrap();
    }

    let addrs = repo.get_client_off_addresses("dc1").await.unwrap();
    assert_eq!(addrs.len(), 5);
}

// ===========================================================================
// Pool and migration tests
// ===========================================================================

#[tokio::test]
async fn pool_creation_and_migration_succeeds() {
    let pool = create_pool("sqlite::memory:")
        .await
        .expect("pool should be created");
    run_migrations(&pool)
        .await
        .expect("migrations should succeed");
}

#[tokio::test]
async fn migrations_are_idempotent() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    run_migrations(&pool).await.unwrap();
    // Running migrations again should not fail because of CREATE TABLE IF NOT EXISTS
    run_migrations(&pool)
        .await
        .expect("second migration run should succeed");
}
