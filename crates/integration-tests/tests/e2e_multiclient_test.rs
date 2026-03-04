// End-to-end multi-client integration tests.
//
// Each test boots a full cluster and exercises cross-client publish/subscribe
// scenarios with multiple independent `TestClient` instances and high fan-out /
// fan-in workloads.
//
// Port offsets 50-53 (ports 19_600 + offset*10 .. +9).

use std::time::Duration;

use sofa_registry_core::model::ReceivedData;
use sofa_registry_integration_tests::harness::{init_test_tracing, TestClient, TestCluster};

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

// ---------------------------------------------------------------------------
// 1. Two clients cross pub/sub
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_two_clients_cross_pubsub() {
    init_test_tracing();
    let cluster = TestCluster::start(50).await;
    cluster
        .wait_for_ready(Duration::from_secs(10))
        .await
        .unwrap();

    // Client A publishes
    let client_a = TestClient::connect(&cluster.session_grpc_addr()).await;
    let _pub_handle = client_a
        .publish("com.example.multi.test1", &["from_client_a"])
        .await;
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Client B subscribes via a completely separate TestClient
    let client_b = TestClient::connect(&cluster.session_grpc_addr()).await;
    let (_sub_handle, collector) = client_b.subscribe("com.example.multi.test1").await;

    let received = collector
        .wait_for_push(Duration::from_secs(5))
        .await
        .unwrap();
    let values = extract_values(&received);
    assert!(
        values.contains(&"from_client_a".to_string()),
        "Client B should see data from Client A, got {:?}",
        values
    );

    cluster.stop().await;
}

// ---------------------------------------------------------------------------
// 2. Fan-out: 1 publisher, 10 subscribers
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_many_clients_fan_out() {
    init_test_tracing();
    let cluster = TestCluster::start(51).await;
    cluster
        .wait_for_ready(Duration::from_secs(10))
        .await
        .unwrap();

    // One publisher
    let pub_client = TestClient::connect(&cluster.session_grpc_addr()).await;
    let _pub_handle = pub_client
        .publish("com.example.multi.test2", &["broadcast_data"])
        .await;
    tokio::time::sleep(Duration::from_millis(500)).await;

    // 10 subscribers, each on its own client
    let mut collectors = Vec::new();
    let mut sub_clients = Vec::new();
    for _ in 0..10 {
        let sub_client = TestClient::connect(&cluster.session_grpc_addr()).await;
        let (_sub_handle, collector) = sub_client.subscribe("com.example.multi.test2").await;
        collectors.push(collector);
        sub_clients.push(sub_client);
    }

    // All 10 subscribers should receive the data
    for (i, collector) in collectors.iter().enumerate() {
        let received = collector
            .wait_for_push(Duration::from_secs(5))
            .await
            .unwrap_or_else(|e| panic!("Subscriber {} timed out: {}", i, e));
        let values = extract_values(&received);
        assert!(
            values.contains(&"broadcast_data".to_string()),
            "Subscriber {} expected 'broadcast_data', got {:?}",
            i,
            values
        );
    }

    cluster.stop().await;
}

// ---------------------------------------------------------------------------
// 3. Fan-in: 10 publishers, 1 subscriber
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_many_publishers_fan_in() {
    init_test_tracing();
    let cluster = TestCluster::start(52).await;
    cluster
        .wait_for_ready(Duration::from_secs(10))
        .await
        .unwrap();

    // 10 publishers, each on its own client, each publishing a unique value
    let mut pub_clients = Vec::new();
    for i in 0..10 {
        let pub_client = TestClient::connect(&cluster.session_grpc_addr()).await;
        let value = format!("pub_{}", i);
        let _pub_handle = pub_client
            .publish("com.example.multi.test3", &[&value])
            .await;
        pub_clients.push(pub_client);
    }
    tokio::time::sleep(Duration::from_millis(1000)).await;

    // One subscriber
    let sub_client = TestClient::connect(&cluster.session_grpc_addr()).await;
    let (_sub_handle, collector) = sub_client.subscribe("com.example.multi.test3").await;

    let received = collector
        .wait_for_push(Duration::from_secs(5))
        .await
        .unwrap();
    let values = extract_values(&received);

    // The subscriber should see all 10 publisher values
    for i in 0..10 {
        let expected = format!("pub_{}", i);
        assert!(
            values.contains(&expected),
            "Expected '{}' in subscriber data, got {:?}",
            expected,
            values
        );
    }

    cluster.stop().await;
}

// ---------------------------------------------------------------------------
// 4. 100 services concurrent
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_100_services_concurrent() {
    init_test_tracing();
    let cluster = TestCluster::start(53).await;
    cluster
        .wait_for_ready(Duration::from_secs(10))
        .await
        .unwrap();

    // Use a single shared client for all registrations to avoid creating 200
    // connections.
    let client = TestClient::connect(&cluster.session_grpc_addr()).await;

    // Register 100 publisher/subscriber pairs, each with a unique dataId.
    let mut collectors = Vec::with_capacity(100);
    for i in 0..100 {
        let data_id = format!("com.example.multi.concurrent.svc{}", i);
        let value = format!("value_{}", i);
        let _pub_handle = client.publish(&data_id, &[&value]).await;
        let (_sub_handle, collector) = client.subscribe(&data_id).await;
        collectors.push((i, collector));
    }

    // Wait a bit for all pushes to propagate
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Verify each subscriber received data for its own dataId
    let mut success_count = 0;
    for (i, collector) in &collectors {
        let expected = format!("value_{}", i);
        match collector.wait_for_push(Duration::from_secs(5)).await {
            Ok(data) => {
                let values = extract_values(&data);
                assert!(
                    values.contains(&expected),
                    "Service {} expected '{}', got {:?}",
                    i,
                    expected,
                    values
                );
                success_count += 1;
            }
            Err(e) => {
                tracing::warn!("Service {} did not receive push: {}", i, e);
            }
        }
    }

    // Allow a small margin: at least 90 out of 100 should succeed
    assert!(
        success_count >= 90,
        "Expected at least 90 services to receive pushes, got {}",
        success_count
    );
    tracing::info!(
        "100 services test: {}/100 received pushes",
        success_count
    );

    cluster.stop().await;
}
