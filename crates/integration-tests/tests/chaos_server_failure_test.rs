// Chaos tests for server failure and recovery scenarios.
//
// These tests verify that the registry cluster can survive individual server
// crashes and restarts, maintaining or recovering pub/sub functionality.
//
// Port offsets 70-73 (ports 19700-19739).

use std::time::Duration;

use sofa_registry_integration_tests::harness::{
    http_get_json, init_test_tracing, TestCluster, TestClient,
};

// ---------------------------------------------------------------------------
// Test 1: Data server crash and restart (offset 70)
// ---------------------------------------------------------------------------
#[tokio::test(flavor = "multi_thread")]
async fn test_data_server_crash_and_restart() {
    init_test_tracing();

    // Boot the full cluster.
    let mut cluster = TestCluster::start(70).await;
    cluster
        .wait_for_ready(Duration::from_secs(15))
        .await
        .expect("cluster should become ready");

    // Publish and subscribe.
    let client = TestClient::connect(&cluster.session_grpc_addr()).await;
    let data_id = "chaos.data.restart.test";

    let (_sub_handle, collector) = client.subscribe(data_id).await;
    tokio::time::sleep(Duration::from_millis(300)).await;

    let _pub_handle = client.publish(data_id, &["value-before-crash"]).await;

    // Wait for the subscriber to receive the initial push.
    let initial = collector.wait_for_push(Duration::from_secs(10)).await;
    assert!(initial.is_ok(), "subscriber should receive initial data push");

    // --- Crash and restart the Data server ---
    cluster.restart_data_server().await;
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Verify the data server is healthy again.
    let health = http_get_json(&format!("{}/api/data/health", cluster.data_http_url())).await;
    assert_eq!(health["status"].as_str().unwrap(), "UP");

    // Publish new data after restart. A new client is needed since the
    // session server may need to re-route to the restarted data server.
    let client2 = TestClient::connect(&cluster.session_grpc_addr()).await;
    let (_sub2, collector2) = client2.subscribe(data_id).await;
    tokio::time::sleep(Duration::from_millis(300)).await;

    let _pub2 = client2.publish(data_id, &["value-after-restart"]).await;

    let after_restart = collector2.wait_for_push(Duration::from_secs(10)).await;
    assert!(
        after_restart.is_ok(),
        "subscriber should receive data after data server restart"
    );

    client.shutdown();
    client2.shutdown();
    cluster.stop().await;
}

// ---------------------------------------------------------------------------
// Test 2: Session server crash and restart (offset 71)
// ---------------------------------------------------------------------------
#[tokio::test(flavor = "multi_thread")]
async fn test_session_server_crash_and_restart() {
    init_test_tracing();

    let mut cluster = TestCluster::start(71).await;
    cluster
        .wait_for_ready(Duration::from_secs(15))
        .await
        .expect("cluster should become ready");

    // Establish initial pub/sub to confirm the cluster works.
    let client1 = TestClient::connect(&cluster.session_grpc_addr()).await;
    let data_id = "chaos.session.restart.test";

    let _pub1 = client1.publish(data_id, &["before-session-crash"]).await;
    let (_sub1, collector1) = client1.subscribe(data_id).await;

    let initial = collector1.wait_for_push(Duration::from_secs(10)).await;
    assert!(
        initial.is_ok(),
        "initial pub/sub should work before session restart"
    );

    // Drop the old client before restarting session to avoid dangling
    // connections.
    client1.shutdown();
    tokio::time::sleep(Duration::from_millis(200)).await;

    // --- Restart session server ---
    cluster.restart_session_server().await;
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Verify session health.
    let health =
        http_get_json(&format!("{}/api/session/health", cluster.session_http_url())).await;
    assert_eq!(health["status"].as_str().unwrap(), "UP");

    // Create a new client against the restarted session server.
    let client2 = TestClient::connect(&cluster.session_grpc_addr()).await;

    let _pub2 = client2.publish(data_id, &["after-session-restart"]).await;
    let (_sub2, collector2) = client2.subscribe(data_id).await;

    let after_restart = collector2.wait_for_push(Duration::from_secs(10)).await;
    assert!(
        after_restart.is_ok(),
        "pub/sub should work after session server restart"
    );

    client2.shutdown();
    cluster.stop().await;
}

// ---------------------------------------------------------------------------
// Test 3: Meta server crash and restart (offset 72)
// ---------------------------------------------------------------------------
#[tokio::test(flavor = "multi_thread")]
async fn test_meta_server_crash_and_restart() {
    init_test_tracing();

    let mut cluster = TestCluster::start(72).await;
    cluster
        .wait_for_ready(Duration::from_secs(15))
        .await
        .expect("cluster should become ready");

    // Verify meta is healthy and has a leader.
    let health_before =
        http_get_json(&format!("{}/api/meta/health", cluster.meta_http_url())).await;
    assert_eq!(health_before["status"].as_str().unwrap(), "UP");

    // --- Restart meta server ---
    cluster.restart_meta_server().await;

    // Wait for leader re-election to complete. The meta server needs to
    // acquire the distributed lock again.
    tokio::time::sleep(Duration::from_secs(5)).await;

    // Poll until meta reports UP again (leader elected).
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap();
    let health_url = format!("{}/api/meta/health", cluster.meta_http_url());

    loop {
        if tokio::time::Instant::now() >= deadline {
            panic!("Meta server did not become UP after restart within timeout");
        }

        if let Ok(resp) = client.get(&health_url).send().await {
            if let Ok(body) = resp.json::<serde_json::Value>().await {
                if body.get("status").and_then(|s| s.as_str()) == Some("UP") {
                    break;
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    // Verify leader endpoint also works after restart.
    let leader = http_get_json(&format!("{}/api/meta/leader", cluster.meta_http_url())).await;
    assert!(
        leader.get("leader").is_some(),
        "leader endpoint should return leader info after restart"
    );

    cluster.stop().await;
}

// ---------------------------------------------------------------------------
// Test 4: All servers restart in sequence (offset 73)
// ---------------------------------------------------------------------------
#[tokio::test(flavor = "multi_thread")]
async fn test_all_servers_restart_in_sequence() {
    init_test_tracing();

    let mut cluster = TestCluster::start(73).await;
    cluster
        .wait_for_ready(Duration::from_secs(15))
        .await
        .expect("cluster should become ready");

    // Confirm pub/sub works initially.
    let client1 = TestClient::connect(&cluster.session_grpc_addr()).await;
    let data_id = "chaos.all.restart.test";

    let _pub1 = client1.publish(data_id, &["before-cascade"]).await;
    let (_sub1, collector1) = client1.subscribe(data_id).await;

    let initial = collector1.wait_for_push(Duration::from_secs(10)).await;
    assert!(initial.is_ok(), "initial pub/sub should work");
    client1.shutdown();

    // --- Restart Meta ---
    cluster.restart_meta_server().await;
    tokio::time::sleep(Duration::from_secs(5)).await;

    // Wait for meta to be UP again.
    let meta_ready_deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    let http_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap();
    let meta_health_url = format!("{}/api/meta/health", cluster.meta_http_url());
    loop {
        if tokio::time::Instant::now() >= meta_ready_deadline {
            panic!("Meta server did not recover after restart");
        }
        if let Ok(resp) = http_client.get(&meta_health_url).send().await {
            if let Ok(body) = resp.json::<serde_json::Value>().await {
                if body.get("status").and_then(|s| s.as_str()) == Some("UP") {
                    break;
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    // --- Restart Data ---
    cluster.restart_data_server().await;
    tokio::time::sleep(Duration::from_secs(2)).await;

    let data_health = http_get_json(&format!("{}/api/data/health", cluster.data_http_url())).await;
    assert_eq!(data_health["status"].as_str().unwrap(), "UP");

    // --- Restart Session ---
    cluster.restart_session_server().await;
    tokio::time::sleep(Duration::from_secs(2)).await;

    let session_health =
        http_get_json(&format!("{}/api/session/health", cluster.session_http_url())).await;
    assert_eq!(session_health["status"].as_str().unwrap(), "UP");

    // --- Verify pub/sub still works after all restarts ---
    let client2 = TestClient::connect(&cluster.session_grpc_addr()).await;

    let _pub2 = client2.publish(data_id, &["after-cascade"]).await;
    let (_sub2, collector2) = client2.subscribe(data_id).await;

    let after_cascade = collector2.wait_for_push(Duration::from_secs(10)).await;
    assert!(
        after_cascade.is_ok(),
        "pub/sub should work after all servers restarted in sequence"
    );

    client2.shutdown();
    cluster.stop().await;
}
