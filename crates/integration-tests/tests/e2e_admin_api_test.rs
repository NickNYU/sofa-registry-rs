// End-to-end tests for admin HTTP API endpoints.
//
// These tests verify that the admin APIs on each server role (Session, Data,
// Meta) correctly reflect the state of the cluster after client registrations.
//
// Port offsets 64-68 are reserved for this test file.

use std::time::Duration;

use sofa_registry_integration_tests::harness::{http_get_json, init_test_tracing, TestCluster};

// ---------------------------------------------------------------------------
// Test 1: Session publisher count reflects registrations (offset 64)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_session_publisher_count_reflects_registrations() {
    init_test_tracing();

    let cluster = TestCluster::start(64).await;
    cluster
        .wait_for_ready(Duration::from_secs(15))
        .await
        .expect("cluster not ready");

    // Before any registrations, publisher count should be 0.
    let url = format!(
        "{}/api/session/publishers/count",
        cluster.session_http_url()
    );
    let body = http_get_json(&url).await;
    assert_eq!(body["count"].as_u64().unwrap(), 0, "initial publisher count should be 0");

    // Register 3 publishers via TestClient.
    let client = sofa_registry_integration_tests::harness::TestClient::connect(
        &cluster.session_grpc_addr(),
    )
    .await;

    let _pub1 = client.publish("com.test.admin.pub.service1", &["v1"]).await;
    let _pub2 = client.publish("com.test.admin.pub.service2", &["v2"]).await;
    let _pub3 = client.publish("com.test.admin.pub.service3", &["v3"]).await;

    // Give the session server time to process registrations.
    tokio::time::sleep(Duration::from_millis(500)).await;

    let body = http_get_json(&url).await;
    assert_eq!(
        body["count"].as_u64().unwrap(),
        3,
        "publisher count should be 3 after registering 3 publishers"
    );
    assert_eq!(
        body["data_info_id_count"].as_u64().unwrap(),
        3,
        "data_info_id_count should be 3 (one per unique dataId)"
    );

    // Also verify via the health endpoint.
    let health_url = format!("{}/api/session/health", cluster.session_http_url());
    let health = http_get_json(&health_url).await;
    assert_eq!(health["publisher_count"].as_u64().unwrap(), 3);

    client.shutdown();
    cluster.stop().await;
}

// ---------------------------------------------------------------------------
// Test 2: Session subscriber count reflects registrations (offset 65)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_session_subscriber_count_reflects_registrations() {
    init_test_tracing();

    let cluster = TestCluster::start(65).await;
    cluster
        .wait_for_ready(Duration::from_secs(15))
        .await
        .expect("cluster not ready");

    let client = sofa_registry_integration_tests::harness::TestClient::connect(
        &cluster.session_grpc_addr(),
    )
    .await;

    // Register 5 subscribers.
    let mut _handles = Vec::new();
    for i in 0..5 {
        let (handle, _collector) = client
            .subscribe(&format!("com.test.admin.sub.service{}", i))
            .await;
        _handles.push((handle, _collector));
    }

    tokio::time::sleep(Duration::from_millis(500)).await;

    let url = format!(
        "{}/api/session/subscribers/count",
        cluster.session_http_url()
    );
    let body = http_get_json(&url).await;
    assert_eq!(
        body["count"].as_u64().unwrap(),
        5,
        "subscriber count should be 5 after registering 5 subscribers"
    );
    assert_eq!(
        body["data_info_id_count"].as_u64().unwrap(),
        5,
        "data_info_id_count should be 5 (one per unique dataId)"
    );

    // Also verify via health endpoint.
    let health_url = format!("{}/api/session/health", cluster.session_http_url());
    let health = http_get_json(&health_url).await;
    assert_eq!(health["subscriber_count"].as_u64().unwrap(), 5);

    client.shutdown();
    cluster.stop().await;
}

// ---------------------------------------------------------------------------
// Test 3: Data datum count reflects publishes (offset 66)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_data_datum_count_reflects_publishes() {
    init_test_tracing();

    let cluster = TestCluster::start(66).await;
    cluster
        .wait_for_ready(Duration::from_secs(15))
        .await
        .expect("cluster not ready");

    let client = sofa_registry_integration_tests::harness::TestClient::connect(
        &cluster.session_grpc_addr(),
    )
    .await;

    // Register publishers for 3 different dataIds.
    let _pub1 = client
        .publish("com.test.admin.datum.serviceA", &["dataA"])
        .await;
    let _pub2 = client
        .publish("com.test.admin.datum.serviceB", &["dataB"])
        .await;
    let _pub3 = client
        .publish("com.test.admin.datum.serviceC", &["dataC"])
        .await;

    // Wait for data to propagate from session to data server.
    tokio::time::sleep(Duration::from_secs(3)).await;

    let url = format!("{}/api/data/datum/count", cluster.data_http_url());
    let body = http_get_json(&url).await;

    // datum_count should reflect the number of distinct dataIds that have publishers.
    let datum_count = body["datum_count"].as_u64().unwrap();
    assert!(
        datum_count >= 3,
        "datum_count should be >= 3 after publishing to 3 dataIds, got {}",
        datum_count
    );

    let publisher_count = body["publisher_count"].as_u64().unwrap();
    assert!(
        publisher_count >= 3,
        "publisher_count should be >= 3, got {}",
        publisher_count
    );

    client.shutdown();
    cluster.stop().await;
}

// ---------------------------------------------------------------------------
// Test 4: Meta slot table has all configured slots (offset 67)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_meta_slot_table_has_all_slots() {
    init_test_tracing();

    // Boot a cluster with slot_num=16 (the default for tests).
    let cluster = TestCluster::start_with_config(67, |cfg| {
        cfg.meta.slot_num = 16;
    })
    .await;
    cluster
        .wait_for_ready(Duration::from_secs(15))
        .await
        .expect("cluster not ready");

    let url = format!("{}/api/meta/slot/table", cluster.meta_http_url());
    let body = http_get_json(&url).await;

    // The slot table is serialized as { "epoch": ..., "slots": { "0": {...}, "1": {...}, ... } }.
    let slots = body["slots"].as_object().expect("slots should be an object");
    assert_eq!(
        slots.len(),
        16,
        "slot table should have exactly 16 slots, got {}",
        slots.len()
    );

    // Verify epoch is positive (assigned after leader election).
    let epoch = body["epoch"].as_i64().unwrap();
    assert!(epoch > 0, "slot table epoch should be positive, got {}", epoch);

    cluster.stop().await;
}

// ---------------------------------------------------------------------------
// Test 5: Meta shows registered data and session servers (offset 68)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_meta_shows_registered_servers() {
    init_test_tracing();

    let cluster = TestCluster::start(68).await;
    cluster
        .wait_for_ready(Duration::from_secs(15))
        .await
        .expect("cluster not ready");

    // Check meta health endpoint for server counts.
    let health_url = format!("{}/api/meta/health", cluster.meta_http_url());
    let health = http_get_json(&health_url).await;

    let data_count = health["data_server_count"].as_u64().unwrap();
    let session_count = health["session_server_count"].as_u64().unwrap();

    assert!(
        data_count >= 1,
        "data_server_count should be >= 1, got {}",
        data_count
    );
    assert!(
        session_count >= 1,
        "session_server_count should be >= 1, got {}",
        session_count
    );

    // Also verify via the dedicated node list endpoints.
    let data_nodes_url = format!("{}/api/meta/nodes/data", cluster.meta_http_url());
    let data_nodes = http_get_json(&data_nodes_url).await;
    let node_count = data_nodes["count"].as_u64().unwrap();
    assert!(
        node_count >= 1,
        "data nodes count should be >= 1, got {}",
        node_count
    );
    let nodes_array = data_nodes["nodes"].as_array().expect("nodes should be an array");
    assert!(
        !nodes_array.is_empty(),
        "data nodes list should not be empty"
    );

    let session_nodes_url = format!("{}/api/meta/nodes/session", cluster.meta_http_url());
    let session_nodes = http_get_json(&session_nodes_url).await;
    let session_node_count = session_nodes["count"].as_u64().unwrap();
    assert!(
        session_node_count >= 1,
        "session nodes count should be >= 1, got {}",
        session_node_count
    );

    cluster.stop().await;
}
