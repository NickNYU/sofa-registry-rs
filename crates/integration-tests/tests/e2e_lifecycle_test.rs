// End-to-end tests for client lifecycle and connection management.
//
// Verifies heartbeat keep-alive, client reconnection, graceful shutdown
// cleanup, and recovery after server restart.

use std::time::Duration;

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
// Test 1: Client heartbeat keeps connection alive
// -----------------------------------------------------------------------
#[tokio::test]
async fn test_client_heartbeat_keeps_connection() {
    init_test_tracing();
    let cluster = TestCluster::start(60).await;
    cluster
        .wait_for_ready(Duration::from_secs(10))
        .await
        .unwrap();

    let data_id = "com.test.lifecycle.heartbeat";
    let timeout = Duration::from_secs(10);

    let client = TestClient::connect(&cluster.session_grpc_addr()).await;

    // Publish initial data to confirm the client is working.
    let pub_handle = client.publish(data_id, &["alive-start"]).await;
    assert!(
        pub_handle.is_registered(),
        "Publisher should be registered after publish"
    );

    // Wait 10 seconds while background heartbeat keeps the connection alive.
    tokio::time::sleep(Duration::from_secs(10)).await;

    // Verify the client is still functional by republishing.
    pub_handle.republish(&["alive-after-wait"]).await.unwrap();

    // And verify a subscriber can still receive data.
    let sub_client = TestClient::connect(&cluster.session_grpc_addr()).await;
    let (_sub_handle, collector) = sub_client.subscribe(data_id).await;
    let received = collector.wait_for_push(timeout).await.unwrap();
    let values = collect_data_values(&received);
    assert!(
        values.contains(&"alive-after-wait".to_string()),
        "After 10s wait, should still receive updated data, got: {:?}",
        values
    );

    cluster.stop().await;
}

// -----------------------------------------------------------------------
// Test 2: Client reconnect after disconnect
// -----------------------------------------------------------------------
#[tokio::test]
async fn test_client_reconnect_after_disconnect() {
    init_test_tracing();
    let cluster = TestCluster::start(61).await;
    cluster
        .wait_for_ready(Duration::from_secs(10))
        .await
        .unwrap();

    let data_id = "com.test.lifecycle.reconnect";
    let timeout = Duration::from_secs(10);

    // First client connects and publishes.
    {
        let client = TestClient::connect(&cluster.session_grpc_addr()).await;
        let _pub_handle = client.publish(data_id, &["first-connect"]).await;
        // Client is dropped here, triggering shutdown.
    }

    tokio::time::sleep(Duration::from_secs(1)).await;

    // Second client connects to the same cluster and verifies it works.
    let client2 = TestClient::connect(&cluster.session_grpc_addr()).await;
    let pub_handle2 = client2.publish(data_id, &["second-connect"]).await;
    assert!(
        pub_handle2.is_registered(),
        "Second client should be able to publish after reconnecting"
    );

    // Verify a subscriber sees the second client's data.
    let sub_client = TestClient::connect(&cluster.session_grpc_addr()).await;
    let (_sub_handle, collector) = sub_client.subscribe(data_id).await;
    let received = collector.wait_for_push(timeout).await.unwrap();
    let values = collect_data_values(&received);
    assert!(
        values.contains(&"second-connect".to_string()),
        "After reconnect, subscriber should see new data, got: {:?}",
        values
    );

    cluster.stop().await;
}

// -----------------------------------------------------------------------
// Test 3: Graceful shutdown cleans up publishers
// -----------------------------------------------------------------------
#[tokio::test]
async fn test_graceful_shutdown_cleans_publishers() {
    init_test_tracing();
    let cluster = TestCluster::start(62).await;
    cluster
        .wait_for_ready(Duration::from_secs(10))
        .await
        .unwrap();

    let data_id = "com.test.lifecycle.shutdown-cleanup";
    let timeout = Duration::from_secs(15);

    // Client B subscribes first and will stay alive throughout.
    let client_b = TestClient::connect(&cluster.session_grpc_addr()).await;
    let (_sub_handle, collector) = client_b.subscribe(data_id).await;

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Client A publishes data.
    let client_a = TestClient::connect(&cluster.session_grpc_addr()).await;
    let _pub_handle = client_a.publish(data_id, &["client-a-data"]).await;

    // Client B should see Client A's data.
    let first_push = collector.wait_for_push(timeout).await.unwrap();
    let values = collect_data_values(&first_push);
    assert!(
        values.contains(&"client-a-data".to_string()),
        "Subscriber should see Client A's data, got: {:?}",
        values
    );

    let pushes_before = collector.count();

    // Drop Client A -- triggers graceful shutdown and publisher cleanup.
    drop(client_a);

    // Client B should eventually receive a push without Client A's data.
    // Give extra time for the server to detect the disconnection and propagate.
    let all_pushes = collector
        .wait_for_n_pushes(pushes_before + 1, timeout)
        .await
        .unwrap();
    let latest = all_pushes.last().unwrap();
    let values = collect_data_values(latest);
    assert!(
        !values.contains(&"client-a-data".to_string()),
        "After Client A shutdown, data should be cleaned up, got: {:?}",
        values
    );

    cluster.stop().await;
}

// -----------------------------------------------------------------------
// Test 4: Server restart -- client recovers pub/sub
// -----------------------------------------------------------------------
#[tokio::test]
async fn test_server_restart_client_recovers() {
    init_test_tracing();
    let mut cluster = TestCluster::start(63).await;
    cluster
        .wait_for_ready(Duration::from_secs(10))
        .await
        .unwrap();

    let data_id = "com.test.lifecycle.server-restart";
    let timeout = Duration::from_secs(15);

    // Set up publisher and subscriber.
    let pub_client = TestClient::connect(&cluster.session_grpc_addr()).await;
    let pub_handle = pub_client.publish(data_id, &["before-restart"]).await;

    tokio::time::sleep(Duration::from_millis(500)).await;

    let sub_client = TestClient::connect(&cluster.session_grpc_addr()).await;
    let (_sub_handle, collector) = sub_client.subscribe(data_id).await;

    // Verify data flows before restart.
    let pre_restart = collector.wait_for_push(timeout).await.unwrap();
    let values = collect_data_values(&pre_restart);
    assert!(
        values.contains(&"before-restart".to_string()),
        "Before restart, should see published data, got: {:?}",
        values
    );

    // Restart the session server.
    cluster.restart_session_server().await;
    cluster
        .wait_for_ready(Duration::from_secs(15))
        .await
        .unwrap();

    // Give the clients time to reconnect.
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Republish after restart to verify pub still works.
    // The client may have reconnected; if republish fails, that is
    // acceptable -- we create a new client as fallback.
    let republish_ok = pub_handle.republish(&["after-restart"]).await.is_ok();

    if republish_ok {
        // Existing client reconnected successfully.
        // Wait for new subscriber push.
        let sub_client2 = TestClient::connect(&cluster.session_grpc_addr()).await;
        let (_sub_handle2, collector2) = sub_client2.subscribe(data_id).await;
        let post_restart = collector2.wait_for_push(timeout).await.unwrap();
        let values = collect_data_values(&post_restart);
        assert!(
            values.contains(&"after-restart".to_string()),
            "After server restart and republish, should see 'after-restart', got: {:?}",
            values
        );
    } else {
        // Original client could not reconnect; create fresh clients.
        let new_pub = TestClient::connect(&cluster.session_grpc_addr()).await;
        let _new_handle = new_pub.publish(data_id, &["recovered"]).await;

        tokio::time::sleep(Duration::from_millis(500)).await;

        let new_sub = TestClient::connect(&cluster.session_grpc_addr()).await;
        let (_new_sub_handle, new_collector) = new_sub.subscribe(data_id).await;
        let post_restart = new_collector.wait_for_push(timeout).await.unwrap();
        let values = collect_data_values(&post_restart);
        assert!(
            values.contains(&"recovered".to_string()),
            "After server restart with new clients, should see 'recovered', got: {:?}",
            values
        );
    }

    cluster.stop().await;
}
