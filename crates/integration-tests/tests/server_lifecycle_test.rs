// Integration tests for the sofa-registry-rs server lifecycle.
//
// Each test starts one or more servers on unique ports, verifies their HTTP
// health endpoints return the expected "UP" status, and then shuts them down
// cleanly.
//
// IMPORTANT: Tests allocate unique port ranges so they do NOT conflict even when
// run in parallel. Each test group uses a distinct block of 10 ports starting at
// an offset within the 19600-19999 range.

use std::sync::Arc;

use sofa_registry_server_data::config::DataServerConfig;
use sofa_registry_server_data::server::DataServer;
use sofa_registry_server_meta::config::MetaServerConfig;
use sofa_registry_server_meta::server::MetaServer;
use sofa_registry_server_session::config::SessionServerConfig;
use sofa_registry_server_session::server::SessionServer;
use sofa_registry_store::jdbc::{create_pool, run_migrations, SqliteDistributeLockRepo};

/// Port allocator: returns (grpc, http) port pairs for a given test offset.
/// Each test gets a unique offset so no two tests bind the same ports.
///
/// Layout (10 ports per test):
///   offset+0 = meta gRPC
///   offset+1 = meta HTTP
///   offset+2 = data gRPC
///   offset+3 = data HTTP
///   offset+4 = session gRPC
///   offset+5 = session HTTP
struct TestPorts {
    base: u16,
}

impl TestPorts {
    fn new(offset: u16) -> Self {
        Self {
            base: 19600 + offset,
        }
    }

    fn meta_grpc(&self) -> u16 {
        self.base
    }
    fn meta_http(&self) -> u16 {
        self.base + 1
    }
    fn data_grpc(&self) -> u16 {
        self.base + 2
    }
    fn data_http(&self) -> u16 {
        self.base + 3
    }
    fn session_grpc(&self) -> u16 {
        self.base + 4
    }
    fn session_http(&self) -> u16 {
        self.base + 5
    }
}

/// Create a MetaServerConfig backed by a fresh SQLite database in the given
/// temp directory.
fn make_meta_config(ports: &TestPorts, db_path: &std::path::Path) -> MetaServerConfig {
    let db_url = format!("sqlite://{}?mode=rwc", db_path.display());
    MetaServerConfig {
        data_center: "TestDC".to_string(),
        cluster_id: "TestCluster".to_string(),
        local_address: "127.0.0.1".to_string(),
        grpc_port: ports.meta_grpc(),
        http_port: ports.meta_http(),
        meta_peers: vec![format!("127.0.0.1:{}", ports.meta_grpc())],
        db_url,
        session_lease_secs: 30,
        data_lease_secs: 30,
        slot_num: 16, // small for tests
        slot_replicas: 1,
        election_lock_duration_ms: 30_000,
        election_interval_ms: 2_000,
        eviction_interval_secs: 60,
    }
}

fn make_data_config(ports: &TestPorts) -> DataServerConfig {
    DataServerConfig {
        data_center: "TestDC".to_string(),
        cluster_id: "TestCluster".to_string(),
        local_address: "127.0.0.1".to_string(),
        grpc_port: ports.data_grpc(),
        http_port: ports.data_http(),
        meta_server_addresses: vec![format!("127.0.0.1:{}", ports.meta_grpc())],
        slot_sync_interval_secs: 60, // long interval; we don't need sync in tests
        data_change_debounce_ms: 500,
        session_lease_secs: 30,
        slot_num: 16,
    }
}

fn make_session_config(ports: &TestPorts) -> SessionServerConfig {
    SessionServerConfig {
        data_center: "TestDC".to_string(),
        cluster_id: "TestCluster".to_string(),
        local_address: "127.0.0.1".to_string(),
        grpc_port: ports.session_grpc(),
        http_port: ports.session_http(),
        meta_server_addresses: vec![format!("127.0.0.1:{}", ports.meta_grpc())],
        push_task_timeout_ms: 3_000,
        push_task_buffer_size: 1024,
        slot_num: 16,
        connection_idle_timeout_secs: 90,
    }
}

/// Helper: start a MetaServer backed by a fresh in-memory SQLite DB.
async fn start_meta_server(ports: &TestPorts, db_path: &std::path::Path) -> MetaServer {
    let config = make_meta_config(ports, db_path);
    let pool = create_pool(&config.db_url)
        .await
        .expect("failed to create SQLite pool");
    run_migrations(&pool)
        .await
        .expect("failed to run migrations");

    let lock_repo = Arc::new(SqliteDistributeLockRepo::new(pool));

    let server = MetaServer::new(
        config,
        lock_repo
            as Arc<dyn sofa_registry_store::traits::distribute_lock::DistributeLockRepository>,
    )
    .await;
    server.start().await.expect("MetaServer failed to start");
    server
}

/// Helper: start a DataServer.
async fn start_data_server(ports: &TestPorts) -> DataServer {
    let config = make_data_config(ports);
    let mut server = DataServer::new(config);
    server.start().await.expect("DataServer failed to start");
    server
}

/// Helper: start a SessionServer.
async fn start_session_server(ports: &TestPorts) -> SessionServer {
    let config = make_session_config(ports);
    let mut server = SessionServer::new(config);
    server.start().await.expect("SessionServer failed to start");
    server
}

/// Initialize tracing (once per process).
fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_test_writer()
        .try_init();
}

// ---------------------------------------------------------------------------
// Test 1: Meta server starts and responds to health check
// ---------------------------------------------------------------------------
#[tokio::test]
async fn meta_server_starts_and_responds_to_health_check() {
    init_tracing();
    let ports = TestPorts::new(0); // ports 19600-19605
    let tmpdir = tempfile::tempdir().expect("tempdir");
    let db_path = tmpdir.path().join("meta_test1.db");

    let meta = start_meta_server(&ports, &db_path).await;

    // Give the server a moment to bind
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let client = reqwest::Client::new();
    let url = format!("http://127.0.0.1:{}/api/meta/health", ports.meta_http());
    let resp = client
        .get(&url)
        .send()
        .await
        .expect("HTTP request to meta health endpoint failed");

    assert!(resp.status().is_success(), "Expected 2xx from meta /health");

    let body: serde_json::Value = resp.json().await.expect("Failed to parse JSON body");
    // The health check returns "UP" when the leader election has taken hold, or
    // "DOWN" if no leader yet. Either way the server is running and responding.
    let status = body["status"].as_str().expect("missing 'status' field");
    assert!(
        status == "UP" || status == "DOWN",
        "Unexpected status: {}",
        status
    );

    // Verify other expected fields exist
    assert!(body.get("epoch").is_some(), "missing 'epoch' field");
    assert!(
        body.get("data_server_count").is_some(),
        "missing 'data_server_count' field"
    );
    assert!(
        body.get("session_server_count").is_some(),
        "missing 'session_server_count' field"
    );

    meta.stop();
    // Let shutdown propagate
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
}

// ---------------------------------------------------------------------------
// Test 2: Data server starts and responds to health check
// ---------------------------------------------------------------------------
#[tokio::test]
async fn data_server_starts_and_responds_to_health_check() {
    init_tracing();
    let ports = TestPorts::new(10); // ports 19610-19615
    let tmpdir = tempfile::tempdir().expect("tempdir");
    let db_path = tmpdir.path().join("meta_test2.db");

    // Start Meta first (Data needs to register with Meta)
    let meta = start_meta_server(&ports, &db_path).await;
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let data = start_data_server(&ports).await;
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let client = reqwest::Client::new();
    let url = format!("http://127.0.0.1:{}/api/data/health", ports.data_http());
    let resp = client
        .get(&url)
        .send()
        .await
        .expect("HTTP request to data health endpoint failed");

    assert!(resp.status().is_success(), "Expected 2xx from data /health");

    let body: serde_json::Value = resp.json().await.expect("Failed to parse JSON body");
    let status = body["status"].as_str().expect("missing 'status' field");
    assert_eq!(status, "UP", "Data server should always return UP");

    // Verify other expected fields
    assert!(
        body.get("server_type").is_some(),
        "missing 'server_type' field"
    );
    assert!(body.get("uptime_ms").is_some(), "missing 'uptime_ms' field");

    data.shutdown();
    meta.stop();
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
}

// ---------------------------------------------------------------------------
// Test 3: Session server starts and responds to health check
// ---------------------------------------------------------------------------
#[tokio::test]
async fn session_server_starts_and_responds_to_health_check() {
    init_tracing();
    let ports = TestPorts::new(20); // ports 19620-19625
    let tmpdir = tempfile::tempdir().expect("tempdir");
    let db_path = tmpdir.path().join("meta_test3.db");

    // Start Meta first (Session needs to register with Meta)
    let meta = start_meta_server(&ports, &db_path).await;
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let session = start_session_server(&ports).await;
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let client = reqwest::Client::new();
    let url = format!(
        "http://127.0.0.1:{}/api/session/health",
        ports.session_http()
    );
    let resp = client
        .get(&url)
        .send()
        .await
        .expect("HTTP request to session health endpoint failed");

    assert!(
        resp.status().is_success(),
        "Expected 2xx from session /health"
    );

    let body: serde_json::Value = resp.json().await.expect("Failed to parse JSON body");
    let status = body["status"].as_str().expect("missing 'status' field");
    assert_eq!(status, "UP", "Session server should always return UP");

    assert!(
        body.get("publisher_count").is_some(),
        "missing 'publisher_count' field"
    );
    assert!(
        body.get("subscriber_count").is_some(),
        "missing 'subscriber_count' field"
    );
    assert!(
        body.get("connection_count").is_some(),
        "missing 'connection_count' field"
    );

    session.stop();
    meta.stop();
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
}

// ---------------------------------------------------------------------------
// Test 4: Full cluster – all three servers start and respond
// ---------------------------------------------------------------------------
#[tokio::test]
async fn full_cluster_starts_all_three_servers() {
    init_tracing();
    let ports = TestPorts::new(30); // ports 19630-19635
    let tmpdir = tempfile::tempdir().expect("tempdir");
    let db_path = tmpdir.path().join("meta_test4.db");

    // Start in order: Meta -> Data -> Session
    let meta = start_meta_server(&ports, &db_path).await;
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let data = start_data_server(&ports).await;
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let session = start_session_server(&ports).await;
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let client = reqwest::Client::new();

    // Check Meta health
    {
        let url = format!("http://127.0.0.1:{}/api/meta/health", ports.meta_http());
        let resp = client
            .get(&url)
            .send()
            .await
            .expect("meta health request failed");
        assert!(resp.status().is_success());
        let body: serde_json::Value = resp.json().await.expect("meta health json");
        let status = body["status"].as_str().unwrap();
        assert!(
            status == "UP" || status == "DOWN",
            "Unexpected meta status: {}",
            status
        );
    }

    // Check Data health
    {
        let url = format!("http://127.0.0.1:{}/api/data/health", ports.data_http());
        let resp = client
            .get(&url)
            .send()
            .await
            .expect("data health request failed");
        assert!(resp.status().is_success());
        let body: serde_json::Value = resp.json().await.expect("data health json");
        assert_eq!(body["status"].as_str().unwrap(), "UP");
    }

    // Check Session health
    {
        let url = format!(
            "http://127.0.0.1:{}/api/session/health",
            ports.session_http()
        );
        let resp = client
            .get(&url)
            .send()
            .await
            .expect("session health request failed");
        assert!(resp.status().is_success());
        let body: serde_json::Value = resp.json().await.expect("session health json");
        assert_eq!(body["status"].as_str().unwrap(), "UP");
    }

    // Verify additional meta endpoints while we have the full cluster up
    {
        // Version endpoint
        let url = format!("http://127.0.0.1:{}/api/meta/version", ports.meta_http());
        let resp = client.get(&url).send().await.expect("meta version request");
        assert!(resp.status().is_success());
        let body: serde_json::Value = resp.json().await.expect("meta version json");
        assert_eq!(body["server_type"].as_str().unwrap(), "META");
    }

    {
        // Data version endpoint
        let url = format!("http://127.0.0.1:{}/api/data/version", ports.data_http());
        let resp = client.get(&url).send().await.expect("data version request");
        assert!(resp.status().is_success());
        let body: serde_json::Value = resp.json().await.expect("data version json");
        assert_eq!(body["server_type"].as_str().unwrap(), "DATA");
    }

    {
        // Session version endpoint
        let url = format!(
            "http://127.0.0.1:{}/api/session/version",
            ports.session_http()
        );
        let resp = client
            .get(&url)
            .send()
            .await
            .expect("session version request");
        assert!(resp.status().is_success());
        let body: serde_json::Value = resp.json().await.expect("session version json");
        assert_eq!(body["server_type"].as_str().unwrap(), "SESSION");
    }

    // Shut down in reverse order: Session -> Data -> Meta
    session.stop();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    data.shutdown();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    meta.stop();
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
}
