// End-to-end tests for push/streaming behavior.
//
// Verifies that subscribers receive push notifications when publishers
// register, republish, or unregister data.

use std::time::{Duration, Instant};

use sofa_registry_integration_tests::harness::{init_test_tracing, TestCluster, TestClient};

/// Helper: collect all data values from a ReceivedData across all zones.
fn collect_data_values(data: &sofa_registry_core::model::ReceivedData) -> Vec<String> {
    let mut values: Vec<String> = data
        .data
        .values()
        .flat_map(|boxes| {
            boxes
                .iter()
                .filter_map(|b| b.data.clone())
        })
        .collect();
    values.sort();
    values
}

// -----------------------------------------------------------------------
// Test 1: Subscriber gets a push when a publisher registers
// -----------------------------------------------------------------------
#[tokio::test]
async fn test_push_on_publish() {
    init_test_tracing();
    let cluster = TestCluster::start(54).await;
    cluster
        .wait_for_ready(Duration::from_secs(10))
        .await
        .unwrap();

    let data_id = "com.test.push.publish";
    let timeout = Duration::from_secs(10);

    // Subscriber connects and subscribes first.
    let sub_client = TestClient::connect(&cluster.session_grpc_addr()).await;
    let (_sub_handle, collector) = sub_client.subscribe(data_id).await;

    // Give the subscription time to register on the server.
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Publisher registers with data.
    let pub_client = TestClient::connect(&cluster.session_grpc_addr()).await;
    let _pub_handle = pub_client.publish(data_id, &["hello-push"]).await;

    // Subscriber should receive a push containing the published data.
    let received = collector.wait_for_push(timeout).await.unwrap();
    let values = collect_data_values(&received);
    assert!(
        values.contains(&"hello-push".to_string()),
        "Expected push to contain 'hello-push', got: {:?}",
        values
    );

    cluster.stop().await;
}

// -----------------------------------------------------------------------
// Test 2: Subscriber sees updated data after republish
// -----------------------------------------------------------------------
#[tokio::test]
async fn test_push_on_republish() {
    init_test_tracing();
    let cluster = TestCluster::start(55).await;
    cluster
        .wait_for_ready(Duration::from_secs(10))
        .await
        .unwrap();

    let data_id = "com.test.push.republish";
    let timeout = Duration::from_secs(10);

    // Publisher registers with v1.
    let pub_client = TestClient::connect(&cluster.session_grpc_addr()).await;
    let pub_handle = pub_client.publish(data_id, &["v1"]).await;

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Subscriber subscribes and should get v1.
    let sub_client = TestClient::connect(&cluster.session_grpc_addr()).await;
    let (_sub_handle, collector) = sub_client.subscribe(data_id).await;

    let first_push = collector.wait_for_push(timeout).await.unwrap();
    let values = collect_data_values(&first_push);
    assert!(
        values.contains(&"v1".to_string()),
        "First push should contain 'v1', got: {:?}",
        values
    );

    let pushes_before = collector.count();

    // Republish with v2.
    pub_handle.republish(&["v2"]).await.unwrap();

    // Wait for the updated push.
    let all_pushes = collector
        .wait_for_n_pushes(pushes_before + 1, timeout)
        .await
        .unwrap();
    let latest = all_pushes.last().unwrap();
    let values = collect_data_values(latest);
    assert!(
        values.contains(&"v2".to_string()),
        "Updated push should contain 'v2', got: {:?}",
        values
    );

    cluster.stop().await;
}

// -----------------------------------------------------------------------
// Test 3: Subscriber gets push after publisher unregisters
// -----------------------------------------------------------------------
#[tokio::test]
async fn test_push_on_unpublish() {
    init_test_tracing();
    let cluster = TestCluster::start(56).await;
    cluster
        .wait_for_ready(Duration::from_secs(10))
        .await
        .unwrap();

    let data_id = "com.test.push.unpublish";
    let timeout = Duration::from_secs(10);

    // Publisher registers.
    let pub_client = TestClient::connect(&cluster.session_grpc_addr()).await;
    let pub_handle = pub_client.publish(data_id, &["will-remove"]).await;

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Subscriber subscribes and gets initial data.
    let sub_client = TestClient::connect(&cluster.session_grpc_addr()).await;
    let (_sub_handle, collector) = sub_client.subscribe(data_id).await;

    let initial = collector.wait_for_push(timeout).await.unwrap();
    let values = collect_data_values(&initial);
    assert!(
        values.contains(&"will-remove".to_string()),
        "Initial push should contain 'will-remove', got: {:?}",
        values
    );

    let pushes_before = collector.count();

    // Unregister the publisher.
    pub_handle.unregister().await.unwrap();

    // Wait for a new push that should no longer contain the publisher's data.
    let all_pushes = collector
        .wait_for_n_pushes(pushes_before + 1, timeout)
        .await
        .unwrap();
    let latest = all_pushes.last().unwrap();
    let values = collect_data_values(latest);
    assert!(
        !values.contains(&"will-remove".to_string()),
        "After unpublish, data should not contain 'will-remove', got: {:?}",
        values
    );

    cluster.stop().await;
}

// -----------------------------------------------------------------------
// Test 4: Push contains data from all publishers
// -----------------------------------------------------------------------
#[tokio::test]
async fn test_push_data_contains_all_publishers() {
    init_test_tracing();
    let cluster = TestCluster::start(57).await;
    cluster
        .wait_for_ready(Duration::from_secs(10))
        .await
        .unwrap();

    let data_id = "com.test.push.multi";
    let timeout = Duration::from_secs(10);

    // Two publishers register the same dataId with different values.
    let pub_a = TestClient::connect(&cluster.session_grpc_addr()).await;
    let _handle_a = pub_a.publish(data_id, &["value-A"]).await;

    let pub_b = TestClient::connect(&cluster.session_grpc_addr()).await;
    let _handle_b = pub_b.publish(data_id, &["value-B"]).await;

    tokio::time::sleep(Duration::from_secs(1)).await;

    // Subscriber should see both values.
    let sub_client = TestClient::connect(&cluster.session_grpc_addr()).await;
    let (_sub_handle, collector) = sub_client.subscribe(data_id).await;

    let received = collector.wait_for_push(timeout).await.unwrap();
    let values = collect_data_values(&received);
    assert!(
        values.contains(&"value-A".to_string()),
        "Push should contain 'value-A', got: {:?}",
        values
    );
    assert!(
        values.contains(&"value-B".to_string()),
        "Push should contain 'value-B', got: {:?}",
        values
    );

    cluster.stop().await;
}

// -----------------------------------------------------------------------
// Test 5: Push version increases monotonically across publishes
// -----------------------------------------------------------------------
#[tokio::test]
async fn test_push_version_monotonically_increases() {
    init_test_tracing();
    let cluster = TestCluster::start(58).await;
    cluster
        .wait_for_ready(Duration::from_secs(10))
        .await
        .unwrap();

    let data_id = "com.test.push.version";
    let timeout = Duration::from_secs(10);

    // Subscribe first.
    let sub_client = TestClient::connect(&cluster.session_grpc_addr()).await;
    let (_sub_handle, collector) = sub_client.subscribe(data_id).await;

    tokio::time::sleep(Duration::from_secs(1)).await;

    // Publish 3 times in sequence.
    let pub_client = TestClient::connect(&cluster.session_grpc_addr()).await;
    let pub_handle = pub_client.publish(data_id, &["iter-1"]).await;

    // Wait for first push.
    collector.wait_for_push(timeout).await.unwrap();

    let count_after_1 = collector.count();
    pub_handle.republish(&["iter-2"]).await.unwrap();
    collector
        .wait_for_n_pushes(count_after_1 + 1, timeout)
        .await
        .unwrap();

    let count_after_2 = collector.count();
    pub_handle.republish(&["iter-3"]).await.unwrap();
    collector
        .wait_for_n_pushes(count_after_2 + 1, timeout)
        .await
        .unwrap();

    // Collect all pushes and verify versions increase monotonically.
    let all = collector
        .wait_for_n_pushes(3, timeout)
        .await
        .unwrap();

    let versions: Vec<Option<i64>> = all.iter().map(|d| d.version).collect();

    // Verify that each successive version is >= the previous (monotonic).
    for window in versions.windows(2) {
        if let (Some(prev), Some(next)) = (window[0], window[1]) {
            assert!(
                next >= prev,
                "Version should increase monotonically: {:?}",
                versions
            );
        }
    }

    // At least verify we got 3+ pushes.
    assert!(
        all.len() >= 3,
        "Expected at least 3 pushes, got {}",
        all.len()
    );

    cluster.stop().await;
}

// -----------------------------------------------------------------------
// Test 6: Push latency is under threshold (5 seconds)
// -----------------------------------------------------------------------
#[tokio::test]
async fn test_push_latency_under_threshold() {
    init_test_tracing();
    let cluster = TestCluster::start(59).await;
    cluster
        .wait_for_ready(Duration::from_secs(10))
        .await
        .unwrap();

    let data_id = "com.test.push.latency";
    let timeout = Duration::from_secs(10);
    let max_latency = Duration::from_secs(5);

    // Subscribe first.
    let sub_client = TestClient::connect(&cluster.session_grpc_addr()).await;
    let (_sub_handle, collector) = sub_client.subscribe(data_id).await;

    tokio::time::sleep(Duration::from_secs(1)).await;

    // Measure the time between publish and push receipt.
    let pub_client = TestClient::connect(&cluster.session_grpc_addr()).await;
    let start = Instant::now();
    let _pub_handle = pub_client.publish(data_id, &["latency-test"]).await;

    let _received = collector.wait_for_push(timeout).await.unwrap();
    let elapsed = start.elapsed();

    assert!(
        elapsed < max_latency,
        "Push latency {:?} exceeded threshold {:?}",
        elapsed,
        max_latency
    );

    cluster.stop().await;
}
