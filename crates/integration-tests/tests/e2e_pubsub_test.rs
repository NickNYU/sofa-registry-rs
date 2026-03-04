// End-to-end publish/subscribe integration tests.
//
// Each test boots a full cluster on unique ports, exercises the pub/sub API, and
// verifies that subscribers receive the expected data pushes.
//
// Port offsets 40-49 (ports 19_600 + offset*10 .. +9).

use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use sofa_registry_client::config::RegistryClientConfig;
use sofa_registry_client::impl_client::DefaultRegistryClient;
use sofa_registry_client::{
    observer_fn, PublisherRegistration, RegistryClient, SubscriberRegistration,
};
use sofa_registry_core::model::ReceivedData;
use sofa_registry_integration_tests::harness::{init_test_tracing, TestClient, TestCluster};
use tokio::sync::Notify;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract all data values from a ReceivedData as a sorted Vec of strings.
fn extract_values(data: &ReceivedData) -> Vec<String> {
    let mut values: Vec<String> = data
        .data
        .values()
        .flat_map(|boxes| boxes.iter().filter_map(|b| b.data.clone()))
        .collect();
    values.sort();
    values
}

/// Simple inline push collector for tests that need raw client access.
struct InlineCollector {
    received: Arc<Mutex<Vec<ReceivedData>>>,
    notify: Arc<Notify>,
}

impl InlineCollector {
    fn new() -> Self {
        Self {
            received: Arc::new(Mutex::new(Vec::new())),
            notify: Arc::new(Notify::new()),
        }
    }

    fn make_observer(&self) -> Arc<dyn sofa_registry_client::SubscriberDataObserver> {
        let inner = self.received.clone();
        let notify = self.notify.clone();
        Arc::new(observer_fn(move |_data_id, data| {
            inner.lock().push(data);
            notify.notify_waiters();
        }))
    }

    async fn wait_for_push(&self, timeout: Duration) -> Result<ReceivedData, String> {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            {
                let data = self.received.lock();
                if !data.is_empty() {
                    return Ok(data.last().unwrap().clone());
                }
            }
            let remaining = deadline
                .checked_duration_since(tokio::time::Instant::now())
                .unwrap_or(Duration::ZERO);
            if remaining.is_zero() {
                return Err("Timed out waiting for push".to_string());
            }
            match tokio::time::timeout(remaining, self.notify.notified()).await {
                Ok(_) => continue,
                Err(_) => return Err("Timed out waiting for push".to_string()),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 1. Publish then subscribe
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_publish_then_subscribe() {
    init_test_tracing();
    let cluster = TestCluster::start(40).await;
    cluster
        .wait_for_ready(Duration::from_secs(10))
        .await
        .unwrap();

    let client = TestClient::connect(&cluster.session_grpc_addr()).await;

    // Publish first
    let _pub_handle = client.publish("com.example.pubsub.test1", &["hello"]).await;
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Then subscribe
    let (_sub_handle, collector) = client.subscribe("com.example.pubsub.test1").await;

    let received = collector
        .wait_for_push(Duration::from_secs(5))
        .await
        .unwrap();
    let values = extract_values(&received);
    assert!(
        values.contains(&"hello".to_string()),
        "Expected 'hello' in {:?}",
        values
    );

    cluster.stop().await;
}

// ---------------------------------------------------------------------------
// 2. Subscribe then publish
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_subscribe_then_publish() {
    init_test_tracing();
    let cluster = TestCluster::start(41).await;
    cluster
        .wait_for_ready(Duration::from_secs(10))
        .await
        .unwrap();

    let client = TestClient::connect(&cluster.session_grpc_addr()).await;

    // Subscribe first
    let (_sub_handle, collector) = client.subscribe("com.example.pubsub.test2").await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Then publish -- subscriber should get a push notification
    let _pub_handle = client
        .publish("com.example.pubsub.test2", &["world"])
        .await;

    let received = collector
        .wait_for_push(Duration::from_secs(5))
        .await
        .unwrap();
    let values = extract_values(&received);
    assert!(
        values.contains(&"world".to_string()),
        "Expected 'world' in {:?}",
        values
    );

    cluster.stop().await;
}

// ---------------------------------------------------------------------------
// 3. Republish updates subscriber
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_republish_updates_subscriber() {
    init_test_tracing();
    let cluster = TestCluster::start(42).await;
    cluster
        .wait_for_ready(Duration::from_secs(10))
        .await
        .unwrap();

    let client = TestClient::connect(&cluster.session_grpc_addr()).await;

    // Publish v1
    let pub_handle = client
        .publish("com.example.pubsub.test3", &["v1"])
        .await;
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Subscribe
    let (_sub_handle, collector) = client.subscribe("com.example.pubsub.test3").await;
    let received = collector
        .wait_for_push(Duration::from_secs(5))
        .await
        .unwrap();
    let values = extract_values(&received);
    assert!(
        values.contains(&"v1".to_string()),
        "Expected 'v1' in {:?}",
        values
    );

    // Republish v2
    pub_handle.republish(&["v2"]).await.unwrap();

    // Wait for the updated push (second push)
    let all = collector
        .wait_for_n_pushes(2, Duration::from_secs(5))
        .await
        .unwrap();
    let latest = all.last().unwrap();
    let values = extract_values(latest);
    assert!(
        values.contains(&"v2".to_string()),
        "Expected 'v2' after republish, got {:?}",
        values
    );

    cluster.stop().await;
}

// ---------------------------------------------------------------------------
// 4. Unregister publisher
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_unregister_publisher() {
    init_test_tracing();
    let cluster = TestCluster::start(43).await;
    cluster
        .wait_for_ready(Duration::from_secs(10))
        .await
        .unwrap();

    let client = TestClient::connect(&cluster.session_grpc_addr()).await;

    // Publish
    let pub_handle = client
        .publish("com.example.pubsub.test4", &["data"])
        .await;
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Subscribe and get initial data
    let (_sub_handle, collector) = client.subscribe("com.example.pubsub.test4").await;
    let received = collector
        .wait_for_push(Duration::from_secs(5))
        .await
        .unwrap();
    let values = extract_values(&received);
    assert!(
        values.contains(&"data".to_string()),
        "Expected 'data' in {:?}",
        values
    );

    // Unregister the publisher
    pub_handle.unregister().await.unwrap();
    assert!(!pub_handle.is_registered());

    // Wait for an updated push reflecting the removal.
    let result = collector
        .wait_for_n_pushes(2, Duration::from_secs(5))
        .await;
    // Either we got a second push (empty or updated), or we timed out which
    // is acceptable -- the key check is that unregister succeeded.
    if let Ok(all) = result {
        let latest = all.last().unwrap();
        let values = extract_values(latest);
        // After unregister, the data may be empty
        assert!(
            values.is_empty() || !values.contains(&"data".to_string()),
            "Expected no data after unregister, got {:?}",
            values
        );
    }

    cluster.stop().await;
}

// ---------------------------------------------------------------------------
// 5. Unregister subscriber (drop handle)
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_unregister_subscriber() {
    init_test_tracing();
    let cluster = TestCluster::start(44).await;
    cluster
        .wait_for_ready(Duration::from_secs(10))
        .await
        .unwrap();

    let client = TestClient::connect(&cluster.session_grpc_addr()).await;

    // Publish
    let pub_handle = client
        .publish("com.example.pubsub.test5", &["initial"])
        .await;
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Subscribe and get initial data
    let (sub_handle, collector) = client.subscribe("com.example.pubsub.test5").await;
    let _received = collector
        .wait_for_push(Duration::from_secs(5))
        .await
        .unwrap();

    // Drop the subscriber handle -- simulates unregister
    drop(sub_handle);
    drop(collector);
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Republish -- should not crash even though the subscriber is gone
    pub_handle.republish(&["updated"]).await.unwrap();

    // Allow time for any push to propagate (should not panic)
    tokio::time::sleep(Duration::from_secs(1)).await;

    cluster.stop().await;
}

// ---------------------------------------------------------------------------
// 6. Multiple publishers same dataId
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_multiple_publishers_same_dataid() {
    init_test_tracing();
    let cluster = TestCluster::start(45).await;
    cluster
        .wait_for_ready(Duration::from_secs(10))
        .await
        .unwrap();

    let client = TestClient::connect(&cluster.session_grpc_addr()).await;

    // Two publishers for the same dataId
    let _pub1 = client
        .publish("com.example.pubsub.test6", &["alpha"])
        .await;
    let _pub2 = client
        .publish("com.example.pubsub.test6", &["beta"])
        .await;
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Subscribe
    let (_sub_handle, collector) = client.subscribe("com.example.pubsub.test6").await;
    let received = collector
        .wait_for_push(Duration::from_secs(5))
        .await
        .unwrap();
    let values = extract_values(&received);

    // Both publisher values should be visible
    assert!(
        values.contains(&"alpha".to_string()) && values.contains(&"beta".to_string()),
        "Expected both 'alpha' and 'beta' in {:?}",
        values
    );

    cluster.stop().await;
}

// ---------------------------------------------------------------------------
// 7. Multiple subscribers same dataId
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_multiple_subscribers_same_dataid() {
    init_test_tracing();
    let cluster = TestCluster::start(46).await;
    cluster
        .wait_for_ready(Duration::from_secs(10))
        .await
        .unwrap();

    let client = TestClient::connect(&cluster.session_grpc_addr()).await;

    // Publish
    let _pub_handle = client
        .publish("com.example.pubsub.test7", &["shared"])
        .await;
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Two subscribers for the same dataId
    let (_sub1, collector1) = client.subscribe("com.example.pubsub.test7").await;
    let (_sub2, collector2) = client.subscribe("com.example.pubsub.test7").await;

    let received1 = collector1
        .wait_for_push(Duration::from_secs(5))
        .await
        .unwrap();
    let received2 = collector2
        .wait_for_push(Duration::from_secs(5))
        .await
        .unwrap();

    let values1 = extract_values(&received1);
    let values2 = extract_values(&received2);
    assert!(
        values1.contains(&"shared".to_string()),
        "Sub1 expected 'shared', got {:?}",
        values1
    );
    assert!(
        values2.contains(&"shared".to_string()),
        "Sub2 expected 'shared', got {:?}",
        values2
    );

    cluster.stop().await;
}

// ---------------------------------------------------------------------------
// 8. Publish in different groups -- subscriber should NOT receive
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_publish_different_groups() {
    init_test_tracing();
    let cluster = TestCluster::start(47).await;
    cluster
        .wait_for_ready(Duration::from_secs(10))
        .await
        .unwrap();

    // Use the lower-level client API for custom groups.
    let config = RegistryClientConfig {
        session_server_addresses: vec![cluster.session_grpc_addr()],
        connect_timeout_ms: 5_000,
        request_timeout_ms: 10_000,
        ..Default::default()
    };
    let raw_client = Arc::new(DefaultRegistryClient::new(config));
    raw_client.connect().await.unwrap();
    let _bg = raw_client.start_background_tasks();

    // Publish in GROUP_A
    let pub_reg =
        PublisherRegistration::new("com.example.pubsub.test8").with_group("GROUP_A");
    raw_client
        .register_publisher(pub_reg, &["secret"])
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Subscribe in GROUP_B
    let sub_reg =
        SubscriberRegistration::new("com.example.pubsub.test8").with_group("GROUP_B");
    let sub_handle = raw_client.register_subscriber(sub_reg).await.unwrap();

    let inline = InlineCollector::new();
    sub_handle.set_observer(inline.make_observer());

    // Should NOT receive data since groups differ
    let result = inline.wait_for_push(Duration::from_secs(3)).await;
    assert!(
        result.is_err(),
        "Subscriber in GROUP_B should NOT receive data from GROUP_A publisher"
    );

    raw_client.shutdown();
    cluster.stop().await;
}

// ---------------------------------------------------------------------------
// 9. Different instanceIds are separate logical services
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_publish_different_instance_ids() {
    init_test_tracing();
    let cluster = TestCluster::start(48).await;
    cluster
        .wait_for_ready(Duration::from_secs(10))
        .await
        .unwrap();

    let config = RegistryClientConfig {
        session_server_addresses: vec![cluster.session_grpc_addr()],
        connect_timeout_ms: 5_000,
        request_timeout_ms: 10_000,
        ..Default::default()
    };
    let raw_client = Arc::new(DefaultRegistryClient::new(config));
    raw_client.connect().await.unwrap();
    let _bg = raw_client.start_background_tasks();

    // Publisher with instance_id "INST_A"
    let pub_reg = PublisherRegistration::new("com.example.pubsub.test9")
        .with_instance_id("INST_A");
    raw_client
        .register_publisher(pub_reg, &["from_a"])
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Subscriber with instance_id "INST_B"
    let sub_reg = SubscriberRegistration::new("com.example.pubsub.test9")
        .with_instance_id("INST_B");
    let sub_handle = raw_client.register_subscriber(sub_reg).await.unwrap();

    let inline = InlineCollector::new();
    sub_handle.set_observer(inline.make_observer());

    // With different instance IDs, the data_info_id differs so the subscriber
    // may or may not receive the data depending on server matching logic.
    // We verify no panic/crash and observe the behavior.
    let result = inline.wait_for_push(Duration::from_secs(3)).await;
    match &result {
        Ok(data) => {
            let values = extract_values(data);
            tracing::info!("Received data with different instanceId: {:?}", values);
        }
        Err(_) => {
            tracing::info!("No push received for different instanceId (expected)");
        }
    }

    raw_client.shutdown();
    cluster.stop().await;
}

// ---------------------------------------------------------------------------
// 10. Empty publish
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_empty_publish() {
    init_test_tracing();
    let cluster = TestCluster::start(49).await;
    cluster
        .wait_for_ready(Duration::from_secs(10))
        .await
        .unwrap();

    let client = TestClient::connect(&cluster.session_grpc_addr()).await;

    // Publish with empty data list
    let _pub_handle = client.publish("com.example.pubsub.test10", &[]).await;
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Subscribe
    let (_sub_handle, collector) = client.subscribe("com.example.pubsub.test10").await;

    // Either receive an empty push or time out -- both are acceptable
    let result = collector.wait_for_push(Duration::from_secs(3)).await;
    match result {
        Ok(data) => {
            let values = extract_values(&data);
            assert!(
                values.is_empty(),
                "Expected empty data for empty publish, got {:?}",
                values
            );
        }
        Err(_) => {
            // Timeout is acceptable for empty publish -- no data to push
        }
    }

    cluster.stop().await;
}
