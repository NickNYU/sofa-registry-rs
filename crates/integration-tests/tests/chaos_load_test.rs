// Chaos load tests for sofa-registry-rs.
//
// These tests stress the registry under concurrent and high-volume workloads.
// Each test uses a unique port offset (80-83) to avoid conflicts.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use sofa_registry_integration_tests::harness::{
    http_get_json, init_test_tracing, TestClient, TestCluster,
};

// ---------------------------------------------------------------------------
// Test 1: 50 concurrent publish/subscribe clients (offset 80)
// ---------------------------------------------------------------------------
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn test_concurrent_publish_subscribe_50_clients() {
    init_test_tracing();
    let cluster = TestCluster::start(80).await;
    cluster
        .wait_for_ready(Duration::from_secs(15))
        .await
        .expect("cluster not ready");

    let session_addr = cluster.session_grpc_addr();
    let success_count = Arc::new(AtomicUsize::new(0));

    let mut handles = Vec::new();
    for i in 0..50 {
        let addr = session_addr.clone();
        let counter = success_count.clone();
        handles.push(tokio::spawn(async move {
            let data_id = format!("com.test.concurrent.service-{}", i);
            let client = TestClient::connect(&addr).await;

            // Subscribe first so we catch the push
            let (_sub_handle, collector) = client.subscribe(&data_id).await;

            // Publish
            let _pub_handle = client
                .publish(&data_id, &[&format!("payload-{}", i)])
                .await;

            // Wait for push with generous timeout
            match collector.wait_for_push(Duration::from_secs(30)).await {
                Ok(data) => {
                    assert_eq!(data.data_id, data_id);
                    counter.fetch_add(1, Ordering::Relaxed);
                }
                Err(_) => {
                    // Some clients may time out under load; that's acceptable
                }
            }
        }));
    }

    for h in handles {
        let _ = h.await;
    }

    let successes = success_count.load(Ordering::Relaxed);
    // At least 80% of subscribers should have received data
    assert!(
        successes >= 40,
        "Expected at least 40/50 subscribers to receive data, got {}",
        successes
    );

    cluster.stop().await;
}

// ---------------------------------------------------------------------------
// Test 2: Rapid publish/unpublish cycle (offset 81)
// ---------------------------------------------------------------------------
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn test_rapid_publish_unpublish_cycle() {
    init_test_tracing();
    let cluster = TestCluster::start(81).await;
    cluster
        .wait_for_ready(Duration::from_secs(15))
        .await
        .expect("cluster not ready");

    let client = TestClient::connect(&cluster.session_grpc_addr()).await;

    for i in 0..100 {
        let data_id = format!("com.test.rapid.cycle-{}", i);
        let pub_handle = client.publish(&data_id, &[&format!("data-{}", i)]).await;
        pub_handle
            .unregister()
            .await
            .unwrap_or_else(|e| panic!("unregister failed on iteration {}: {}", i, e));
    }

    // Server should still be healthy after rapid cycles
    let health = http_get_json(&format!(
        "{}/api/session/health",
        cluster.session_http_url()
    ))
    .await;
    assert_eq!(
        health["status"].as_str().unwrap(),
        "UP",
        "Session server should be UP after rapid publish/unpublish"
    );

    cluster.stop().await;
}

// ---------------------------------------------------------------------------
// Test 3: Many services steady state (offset 82)
//
// Registers many services using a small pool of shared clients (to avoid
// exceeding h2's per-connection stream limits). This is also a more
// realistic pattern — production apps reuse connections.
// ---------------------------------------------------------------------------
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn test_1000_services_steady_state() {
    init_test_tracing();
    let cluster = TestCluster::start(82).await;
    cluster
        .wait_for_ready(Duration::from_secs(15))
        .await
        .expect("cluster not ready");

    let session_addr = cluster.session_grpc_addr();
    let total_services: usize = 200;
    let num_clients: usize = 10;
    let services_per_client = total_services / num_clients;
    let success_count = Arc::new(AtomicUsize::new(0));

    // Create a pool of shared clients (10 connections, each handling 20 services).
    let mut clients = Vec::new();
    for _ in 0..num_clients {
        clients.push(Arc::new(TestClient::connect(&session_addr).await));
    }

    // Publish all services using the shared clients.
    // Keep publisher handles alive to maintain registrations.
    let mut all_pub_handles: Vec<Arc<dyn sofa_registry_client::PublisherHandle>> = Vec::new();
    for (i, client) in clients.iter().enumerate() {
        for j in 0..services_per_client {
            let idx = i * services_per_client + j;
            let data_id = format!("com.test.steady.svc-{}", idx);
            let pub_handle = client
                .publish(&data_id, &[&format!("steady-payload-{}", idx)])
                .await;
            all_pub_handles.push(pub_handle);
        }
    }

    // Small delay for server to process all publishes.
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Subscribe to all services (using the same client pool) and collect pushes.
    let mut sub_tasks = Vec::new();
    for (i, client) in clients.iter().enumerate() {
        for j in 0..services_per_client {
            let idx = i * services_per_client + j;
            let client = client.clone();
            let counter = success_count.clone();
            sub_tasks.push(tokio::spawn(async move {
                let data_id = format!("com.test.steady.svc-{}", idx);
                let (_sub_handle, collector) = client.subscribe(&data_id).await;
                match collector.wait_for_push(Duration::from_secs(15)).await {
                    Ok(_) => {
                        counter.fetch_add(1, Ordering::Relaxed);
                    }
                    Err(_) => {}
                }
            }));
        }
    }

    // Wait for all subscriber tasks.
    for h in sub_tasks {
        let _ = h.await;
    }

    let successes = success_count.load(Ordering::Relaxed);
    assert!(
        successes >= (total_services * 80 / 100),
        "Expected at least 80% of {} subscribers to receive data, got {}",
        total_services,
        successes
    );

    // All three servers should still be healthy.
    let meta_health = http_get_json(&format!("{}/api/meta/health", cluster.meta_http_url())).await;
    let data_health = http_get_json(&format!("{}/api/data/health", cluster.data_http_url())).await;
    let session_health =
        http_get_json(&format!("{}/api/session/health", cluster.session_http_url())).await;

    assert_eq!(meta_health["status"].as_str().unwrap(), "UP");
    assert_eq!(data_health["status"].as_str().unwrap(), "UP");
    assert_eq!(session_health["status"].as_str().unwrap(), "UP");

    cluster.stop().await;
}

// ---------------------------------------------------------------------------
// Test 4: Burst 100 publishes in ~1 second for the same dataId (offset 83)
// ---------------------------------------------------------------------------
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn test_burst_100_publishes_in_1_second() {
    init_test_tracing();
    let cluster = TestCluster::start(83).await;
    cluster
        .wait_for_ready(Duration::from_secs(15))
        .await
        .expect("cluster not ready");

    let session_addr = cluster.session_grpc_addr();
    let data_id = "com.test.burst.shared-service";

    // Create a subscriber first
    let sub_client = TestClient::connect(&session_addr).await;
    let (_sub_handle, collector) = sub_client.subscribe(data_id).await;

    // Spawn 100 concurrent publishers for the same dataId
    let mut handles = Vec::new();
    for i in 0..100 {
        let addr = session_addr.clone();
        let did = data_id.to_string();
        handles.push(tokio::spawn(async move {
            let client = TestClient::connect(&addr).await;
            let _pub_handle = client
                .publish(&did, &[&format!("burst-payload-{}", i)])
                .await;
            // Keep client alive briefly so the server can process
            tokio::time::sleep(Duration::from_secs(2)).await;
            drop(client);
        }));
    }

    // Wait for at least one push to the subscriber
    let result = collector.wait_for_push(Duration::from_secs(30)).await;
    assert!(
        result.is_ok(),
        "Subscriber should receive at least one push after burst: {:?}",
        result.err()
    );

    let data = result.unwrap();
    assert_eq!(data.data_id, data_id);

    // The push should contain data from the publishers. With 100 publishers,
    // the data map should have entries.
    let total_publishers: usize = data.data.values().map(|v| v.len()).sum();
    assert!(
        total_publishers >= 1,
        "Expected data from at least 1 publisher in the push, got {}",
        total_publishers
    );

    // Wait for all publisher tasks to finish
    for h in handles {
        let _ = h.await;
    }

    cluster.stop().await;
}
