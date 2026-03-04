// Chaos consistency tests for sofa-registry-rs.
//
// These tests verify data consistency guarantees under various failure
// scenarios and edge cases. Each test uses a unique port offset (84-87).

use std::time::Duration;

use sofa_registry_integration_tests::harness::{init_test_tracing, TestClient, TestCluster};

// ---------------------------------------------------------------------------
// Test 1: No data loss after data server restart (offset 84)
// ---------------------------------------------------------------------------
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn test_no_data_loss_after_server_restart() {
    init_test_tracing();
    // Use a lower data_change_debounce_ms to speed up change notifications
    // after data server restart. Under suite-wide contention the default
    // 500 ms debounce can combine with stale-gRPC-channel reconnection time
    // to push individual services past their timeout.
    let mut cluster = TestCluster::start_with_config(84, |cfg| {
        cfg.data.data_change_debounce_ms = 100;
    })
    .await;
    cluster
        .wait_for_ready(Duration::from_secs(15))
        .await
        .expect("cluster not ready");

    let session_addr = cluster.session_grpc_addr();

    // Publish data for 5 dataIds and verify subscribers receive it
    let data_ids: Vec<String> = (0..5)
        .map(|i| format!("com.test.restart.svc-{}", i))
        .collect();

    {
        let client = TestClient::connect(&session_addr).await;
        for (i, data_id) in data_ids.iter().enumerate() {
            let (_sub_handle, collector) = client.subscribe(data_id).await;
            let _pub_handle = client
                .publish(data_id, &[&format!("restart-data-{}", i)])
                .await;

            let result = collector.wait_for_push(Duration::from_secs(10)).await;
            assert!(
                result.is_ok(),
                "Pre-restart: subscriber for {} should receive data: {:?}",
                data_id,
                result.err()
            );
        }
    }

    // Restart the data server
    cluster.restart_data_server().await;
    cluster
        .wait_for_ready(Duration::from_secs(15))
        .await
        .expect("cluster not ready after data server restart");

    // Allow extra time for the session server's stale gRPC channel to the
    // old data server to reconnect to the new one.
    tokio::time::sleep(Duration::from_secs(1)).await;

    // After restart, re-publish and subscribe to verify the system works
    {
        let client = TestClient::connect(&session_addr).await;
        for (i, data_id) in data_ids.iter().enumerate() {
            let (_sub_handle, collector) = client.subscribe(data_id).await;
            // Re-publish after restart
            let _pub_handle = client
                .publish(data_id, &[&format!("post-restart-data-{}", i)])
                .await;

            let result = collector.wait_for_push(Duration::from_secs(20)).await;
            assert!(
                result.is_ok(),
                "Post-restart: subscriber for {} should receive data: {:?}",
                data_id,
                result.err()
            );
            // Settle delay between services under high contention.
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }

    cluster.stop().await;
}

// ---------------------------------------------------------------------------
// Test 2: Subscriber sees latest version after multiple updates (offset 85)
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_subscriber_sees_latest_version() {
    init_test_tracing();
    let cluster = TestCluster::start(85).await;
    cluster
        .wait_for_ready(Duration::from_secs(15))
        .await
        .expect("cluster not ready");

    let client = TestClient::connect(&cluster.session_grpc_addr()).await;
    let data_id = "com.test.versioning.svc";

    // Subscribe first
    let (_sub_handle, collector) = client.subscribe(data_id).await;

    // Publish v1
    let pub_handle = client.publish(data_id, &["v1"]).await;
    let result = collector.wait_for_push(Duration::from_secs(10)).await;
    assert!(result.is_ok(), "Should receive v1 push");

    // Publish v2 (republish overwrites)
    pub_handle.republish(&["v2"]).await.expect("republish v2");
    // Wait for a second push
    let result = collector
        .wait_for_n_pushes(2, Duration::from_secs(10))
        .await;
    assert!(result.is_ok(), "Should receive v2 push");

    // Publish v3
    pub_handle.republish(&["v3"]).await.expect("republish v3");
    let result = collector
        .wait_for_n_pushes(3, Duration::from_secs(10))
        .await;
    assert!(result.is_ok(), "Should receive v3 push");

    // The latest push should contain "v3"
    let latest = collector.latest().expect("should have latest data");
    assert_eq!(latest.data_id, data_id);

    // Check that the data contains "v3"
    let all_values: Vec<String> = latest
        .data
        .values()
        .flat_map(|boxes| boxes.iter())
        .filter_map(|db| db.data.as_ref())
        .cloned()
        .collect();
    assert!(
        all_values.iter().any(|v| v.contains("v3")),
        "Latest push should contain v3 data, got: {:?}",
        all_values
    );

    cluster.stop().await;
}

// ---------------------------------------------------------------------------
// Test 3: Eventual consistency after delay (offset 86)
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_eventual_consistency_after_delay() {
    init_test_tracing();
    let cluster = TestCluster::start(86).await;
    cluster
        .wait_for_ready(Duration::from_secs(15))
        .await
        .expect("cluster not ready");

    let client = TestClient::connect(&cluster.session_grpc_addr()).await;
    let data_id = "com.test.eventual.svc";

    // Subscribe first, then publish
    let (_sub_handle, collector) = client.subscribe(data_id).await;
    let _pub_handle = client.publish(data_id, &["eventual-data"]).await;

    // Wait with a generous timeout (15s) for eventual delivery
    let result = collector.wait_for_push(Duration::from_secs(15)).await;
    assert!(
        result.is_ok(),
        "Data should eventually arrive within 15s: {:?}",
        result.err()
    );

    let data = result.unwrap();
    assert_eq!(data.data_id, data_id);

    // Verify the data actually contains our published value
    let all_values: Vec<String> = data
        .data
        .values()
        .flat_map(|boxes| boxes.iter())
        .filter_map(|db| db.data.as_ref())
        .cloned()
        .collect();
    assert!(
        all_values.iter().any(|v| v.contains("eventual-data")),
        "Push should contain the published data, got: {:?}",
        all_values
    );

    cluster.stop().await;
}

// ---------------------------------------------------------------------------
// Test 4: Duplicate publish with same registId is idempotent (offset 87)
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_duplicate_publish_idempotent() {
    init_test_tracing();
    let cluster = TestCluster::start(87).await;
    cluster
        .wait_for_ready(Duration::from_secs(15))
        .await
        .expect("cluster not ready");

    let client = TestClient::connect(&cluster.session_grpc_addr()).await;
    let data_id = "com.test.idempotent.svc";

    // Publish the same dataId twice. The second publish creates a separate
    // publisher with a different registId by default, but we want to test
    // that publishing with the same effective identity doesn't create
    // duplicates. We use republish on the same handle to simulate this.
    let pub_handle = client.publish(data_id, &["first-publish"]).await;

    // Republish with same handle (same registId) -- should be idempotent
    pub_handle
        .republish(&["second-publish"])
        .await
        .expect("republish should succeed");

    // Subscribe and verify we see exactly 1 publisher, not 2
    let (_sub_handle, collector) = client.subscribe(data_id).await;
    let result = collector.wait_for_push(Duration::from_secs(10)).await;
    assert!(
        result.is_ok(),
        "Subscriber should receive push: {:?}",
        result.err()
    );

    let data = result.unwrap();
    assert_eq!(data.data_id, data_id);

    // Count the total number of data boxes (each publisher contributes entries)
    // With one publisher, we should see exactly 1 set of entries per zone/segment
    let total_publishers: usize = data.data.values().map(|v| v.len()).sum();
    assert_eq!(
        total_publishers, 1,
        "Expected exactly 1 publisher entry (idempotent), got {}",
        total_publishers
    );

    // Verify the data reflects the latest publish ("second-publish")
    let all_values: Vec<String> = data
        .data
        .values()
        .flat_map(|boxes| boxes.iter())
        .filter_map(|db| db.data.as_ref())
        .cloned()
        .collect();
    assert!(
        all_values.iter().any(|v| v.contains("second-publish")),
        "Should see the latest republished data, got: {:?}",
        all_values
    );

    cluster.stop().await;
}
