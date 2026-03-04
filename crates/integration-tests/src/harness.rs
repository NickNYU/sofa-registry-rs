//! Shared test harness for integration and chaos tests.
//!
//! Provides [`TestCluster`] for booting a full Meta + Data + Session cluster on
//! unique ports, and [`TestClient`] for publishing/subscribing against it.

use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use sofa_registry_client::config::RegistryClientConfig;
use sofa_registry_client::impl_client::DefaultRegistryClient;
use sofa_registry_client::{
    observer_fn, PublisherHandle, PublisherRegistration, RegistryClient, SubscriberHandle,
    SubscriberRegistration,
};
use sofa_registry_core::model::ReceivedData;
use sofa_registry_server_data::config::DataServerConfig;
use sofa_registry_server_data::server::DataServer;
use sofa_registry_server_meta::config::MetaServerConfig;
use sofa_registry_server_meta::server::MetaServer;
use sofa_registry_server_session::config::SessionServerConfig;
use sofa_registry_server_session::server::SessionServer;
use sofa_registry_store::jdbc::{create_pool, run_migrations, SqliteDistributeLockRepo};
use tempfile::TempDir;
use tokio::sync::Notify;
use tracing::info;

// ---------------------------------------------------------------------------
// Port allocator
// ---------------------------------------------------------------------------

/// Allocates a block of 10 ports for a test cluster.
///
/// Supports 100+ parallel clusters within the 19600-20600 range.
/// Each cluster gets ports at `19600 + offset * 10`.
pub struct TestPorts {
    base: u16,
}

impl TestPorts {
    /// Create a port block from an offset (0, 1, 2, ...).
    /// Offset 0 uses ports 19600-19609, offset 1 uses 19610-19619, etc.
    pub fn new(offset: u16) -> Self {
        assert!(
            offset < 100,
            "offset must be < 100 to stay within port range 19600-20600"
        );
        Self {
            base: 19600 + offset * 10,
        }
    }

    pub fn meta_grpc(&self) -> u16 {
        self.base
    }
    pub fn meta_http(&self) -> u16 {
        self.base + 1
    }
    pub fn data_grpc(&self) -> u16 {
        self.base + 2
    }
    pub fn data_http(&self) -> u16 {
        self.base + 3
    }
    pub fn session_grpc(&self) -> u16 {
        self.base + 4
    }
    pub fn session_http(&self) -> u16 {
        self.base + 5
    }
}

// ---------------------------------------------------------------------------
// Cluster configuration hook
// ---------------------------------------------------------------------------

/// Mutable configuration bundle exposed to `start_with_config`.
pub struct ClusterConfig {
    pub meta: MetaServerConfig,
    pub data: DataServerConfig,
    pub session: SessionServerConfig,
}

// ---------------------------------------------------------------------------
// TestCluster
// ---------------------------------------------------------------------------

/// A self-contained registry cluster (Meta + Data + Session) for testing.
///
/// Servers are booted in order (Meta -> Data -> Session) and shut down in
/// reverse order when [`stop`] is called.
pub struct TestCluster {
    pub ports: TestPorts,
    meta: Option<MetaServer>,
    data: Option<DataServer>,
    session: Option<SessionServer>,
    // Held to keep the temp directory alive for the cluster's lifetime.
    #[allow(dead_code)]
    tmpdir: TempDir,
    meta_config: MetaServerConfig,
    data_config: DataServerConfig,
    session_config: SessionServerConfig,
    db_path: std::path::PathBuf,
}

impl TestCluster {
    /// Boot a full cluster using the given port offset.
    pub async fn start(port_offset: u16) -> Self {
        Self::start_with_config(port_offset, |_| {}).await
    }

    /// Boot a full cluster, allowing the caller to tweak configuration before
    /// servers are started.
    pub async fn start_with_config(
        port_offset: u16,
        config_fn: impl FnOnce(&mut ClusterConfig),
    ) -> Self {
        let ports = TestPorts::new(port_offset);
        let tmpdir = tempfile::tempdir().expect("failed to create temp dir");
        let db_path = tmpdir.path().join("meta.db");

        let mut cluster_cfg = ClusterConfig {
            meta: Self::default_meta_config(&ports, &db_path),
            data: Self::default_data_config(&ports),
            session: Self::default_session_config(&ports),
        };
        config_fn(&mut cluster_cfg);

        let meta_config = cluster_cfg.meta.clone();
        let data_config = cluster_cfg.data.clone();
        let session_config = cluster_cfg.session.clone();

        // --- Meta ---
        let meta = Self::boot_meta(&cluster_cfg.meta, &db_path).await;
        tokio::time::sleep(Duration::from_millis(500)).await;

        // --- Data ---
        let data = Self::boot_data(&cluster_cfg.data).await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        // --- Session ---
        let session = Self::boot_session(&cluster_cfg.session).await;
        tokio::time::sleep(Duration::from_millis(500)).await;

        Self {
            ports,
            meta: Some(meta),
            data: Some(data),
            session: Some(session),
            tmpdir,
            meta_config,
            data_config,
            session_config,
            db_path,
        }
    }

    // -- URL helpers --------------------------------------------------------

    pub fn meta_http_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.ports.meta_http())
    }

    pub fn data_http_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.ports.data_http())
    }

    pub fn session_http_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.ports.session_http())
    }

    pub fn session_grpc_addr(&self) -> String {
        format!("127.0.0.1:{}", self.ports.session_grpc())
    }

    // -- Readiness ----------------------------------------------------------

    /// Poll health endpoints until all three servers report UP, or the timeout
    /// expires.
    pub async fn wait_for_ready(&self, timeout: Duration) -> Result<(), String> {
        let deadline = tokio::time::Instant::now() + timeout;
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap();

        loop {
            if tokio::time::Instant::now() >= deadline {
                return Err("Timed out waiting for cluster to become ready".to_string());
            }

            let meta_ok = Self::check_health(
                &client,
                &format!("{}/api/meta/health", self.meta_http_url()),
            )
            .await;
            let data_ok = Self::check_health(
                &client,
                &format!("{}/api/data/health", self.data_http_url()),
            )
            .await;
            let session_ok = Self::check_health(
                &client,
                &format!("{}/api/session/health", self.session_http_url()),
            )
            .await;

            if meta_ok && data_ok && session_ok {
                return Ok(());
            }

            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }

    // -- Shutdown -----------------------------------------------------------

    /// Graceful shutdown in reverse order: Session -> Data -> Meta.
    pub async fn stop(mut self) {
        if let Some(session) = self.session.take() {
            session.stop();
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        if let Some(data) = self.data.take() {
            data.shutdown();
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        if let Some(meta) = self.meta.take() {
            meta.stop();
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }

    // -- Chaos helpers: restart individual servers --------------------------

    /// Stop and restart the Data server, preserving the same configuration.
    pub async fn restart_data_server(&mut self) {
        if let Some(data) = self.data.take() {
            info!("Stopping Data server for restart...");
            data.shutdown();
            tokio::time::sleep(Duration::from_millis(300)).await;
        }
        info!("Restarting Data server...");
        let data = Self::boot_data(&self.data_config).await;
        tokio::time::sleep(Duration::from_millis(300)).await;
        self.data = Some(data);
    }

    /// Stop and restart the Session server, preserving the same configuration.
    pub async fn restart_session_server(&mut self) {
        if let Some(mut session) = self.session.take() {
            info!("Stopping Session server for restart...");
            session.stop_and_wait().await;
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        info!("Restarting Session server...");
        let session = Self::boot_session(&self.session_config).await;
        tokio::time::sleep(Duration::from_millis(300)).await;
        self.session = Some(session);
    }

    /// Stop and restart the Meta server, preserving the same configuration and
    /// database.
    pub async fn restart_meta_server(&mut self) {
        if let Some(mut meta) = self.meta.take() {
            info!("Stopping Meta server for restart...");
            meta.stop_and_wait().await;
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        info!("Restarting Meta server...");
        let meta = Self::boot_meta(&self.meta_config, &self.db_path).await;
        tokio::time::sleep(Duration::from_millis(500)).await;
        self.meta = Some(meta);
    }

    // -- Internal helpers ---------------------------------------------------

    fn default_meta_config(ports: &TestPorts, db_path: &std::path::Path) -> MetaServerConfig {
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
            slot_num: 16,
            slot_replicas: 1,
            election_lock_duration_ms: 30_000,
            election_interval_ms: 2_000,
            eviction_interval_secs: 60,
        }
    }

    fn default_data_config(ports: &TestPorts) -> DataServerConfig {
        DataServerConfig {
            data_center: "TestDC".to_string(),
            cluster_id: "TestCluster".to_string(),
            local_address: "127.0.0.1".to_string(),
            grpc_port: ports.data_grpc(),
            http_port: ports.data_http(),
            meta_server_addresses: vec![format!("127.0.0.1:{}", ports.meta_grpc())],
            slot_sync_interval_secs: 60,
            data_change_debounce_ms: 500,
            session_lease_secs: 30,
            slot_num: 16,
        }
    }

    fn default_session_config(ports: &TestPorts) -> SessionServerConfig {
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

    async fn boot_meta(config: &MetaServerConfig, db_path: &std::path::Path) -> MetaServer {
        // Ensure the DB URL points to the right place even after config_fn
        let db_url = format!("sqlite://{}?mode=rwc", db_path.display());
        let mut cfg = config.clone();
        cfg.db_url = db_url.clone();

        let pool = create_pool(&db_url)
            .await
            .expect("failed to create SQLite pool");
        run_migrations(&pool)
            .await
            .expect("failed to run migrations");

        let lock_repo = Arc::new(SqliteDistributeLockRepo::new(pool));
        let mut server = MetaServer::new(
            cfg,
            lock_repo
                as Arc<
                    dyn sofa_registry_store::traits::distribute_lock::DistributeLockRepository,
                >,
        )
        .await;
        server.start().await.expect("MetaServer failed to start");
        server
    }

    async fn boot_data(config: &DataServerConfig) -> DataServer {
        let mut server = DataServer::new(config.clone());
        server.start().await.expect("DataServer failed to start");
        server
    }

    async fn boot_session(config: &SessionServerConfig) -> SessionServer {
        let mut server = SessionServer::new(config.clone());
        server.start().await.expect("SessionServer failed to start");
        server
    }

    async fn check_health(client: &reqwest::Client, url: &str) -> bool {
        match client.get(url).send().await {
            Ok(resp) if resp.status().is_success() => {
                if let Ok(body) = resp.json::<serde_json::Value>().await {
                    body.get("status").and_then(|s| s.as_str()) == Some("UP")
                } else {
                    false
                }
            }
            _ => false,
        }
    }
}

// ---------------------------------------------------------------------------
// TestClient
// ---------------------------------------------------------------------------

/// Convenience wrapper around [`DefaultRegistryClient`] for tests.
pub struct TestClient {
    client: Arc<DefaultRegistryClient>,
    _background: tokio::task::JoinHandle<()>,
}

impl TestClient {
    /// Connect to a session server at `session_grpc_addr` (e.g. `"127.0.0.1:19604"`).
    pub async fn connect(session_grpc_addr: &str) -> Self {
        let config = RegistryClientConfig {
            session_server_addresses: vec![session_grpc_addr.to_string()],
            connect_timeout_ms: 5_000,
            request_timeout_ms: 10_000,
            ..Default::default()
        };
        let client = Arc::new(DefaultRegistryClient::new(config));
        client.connect().await.expect("client connect failed");
        let background = client.start_background_tasks();
        Self {
            client,
            _background: background,
        }
    }

    /// Register a publisher for `data_id` with the given initial data values.
    pub async fn publish(
        &self,
        data_id: &str,
        data: &[&str],
    ) -> Arc<dyn PublisherHandle> {
        let reg = PublisherRegistration::new(data_id);
        self.client
            .register_publisher(reg, data)
            .await
            .expect("publish failed")
    }

    /// Register a subscriber for `data_id` and return its handle plus a
    /// [`ReceivedDataCollector`] that accumulates push notifications.
    pub async fn subscribe(
        &self,
        data_id: &str,
    ) -> (Arc<dyn SubscriberHandle>, ReceivedDataCollector) {
        let reg = SubscriberRegistration::new(data_id);
        let handle = self
            .client
            .register_subscriber(reg)
            .await
            .expect("subscribe failed");

        let collector = ReceivedDataCollector::new();
        let inner = collector.received.clone();
        let notify = collector.notify.clone();
        handle.set_observer(Arc::new(observer_fn(move |_data_id, data| {
            inner.lock().push(data);
            notify.notify_waiters();
        })));

        (handle, collector)
    }

    /// Shut down background tasks.
    pub fn shutdown(&self) {
        self.client.shutdown();
    }
}

impl Drop for TestClient {
    fn drop(&mut self) {
        self.client.shutdown();
    }
}

// ---------------------------------------------------------------------------
// ReceivedDataCollector
// ---------------------------------------------------------------------------

/// Collects push notifications for assertions in tests.
pub struct ReceivedDataCollector {
    received: Arc<Mutex<Vec<ReceivedData>>>,
    notify: Arc<Notify>,
}

impl ReceivedDataCollector {
    fn new() -> Self {
        Self {
            received: Arc::new(Mutex::new(Vec::new())),
            notify: Arc::new(Notify::new()),
        }
    }

    /// Wait until at least one push is received, or time out.
    pub async fn wait_for_push(&self, timeout: Duration) -> Result<ReceivedData, String> {
        let result = self.wait_for_n_pushes(1, timeout).await?;
        Ok(result.into_iter().next().unwrap())
    }

    /// Wait until `n` total pushes have been received, or time out.
    pub async fn wait_for_n_pushes(
        &self,
        n: usize,
        timeout: Duration,
    ) -> Result<Vec<ReceivedData>, String> {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            {
                let data = self.received.lock();
                if data.len() >= n {
                    return Ok(data.clone());
                }
            }
            let remaining = deadline
                .checked_duration_since(tokio::time::Instant::now())
                .unwrap_or(Duration::ZERO);
            if remaining.is_zero() {
                let count = self.received.lock().len();
                return Err(format!(
                    "Timed out waiting for {} pushes (got {})",
                    n, count
                ));
            }
            match tokio::time::timeout(remaining, self.notify.notified()).await {
                Ok(_) => continue,
                Err(_) => {
                    let count = self.received.lock().len();
                    return Err(format!(
                        "Timed out waiting for {} pushes (got {})",
                        n, count
                    ));
                }
            }
        }
    }

    /// Return the most recently received data, if any.
    pub fn latest(&self) -> Option<ReceivedData> {
        self.received.lock().last().cloned()
    }

    /// Number of pushes received so far.
    pub fn count(&self) -> usize {
        self.received.lock().len()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Initialise tracing for tests. Safe to call multiple times; only the first
/// call actually installs the subscriber.
pub fn init_test_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_test_writer()
        .try_init();
}

/// Perform an HTTP GET and parse the response body as JSON.
pub async fn http_get_json(url: &str) -> serde_json::Value {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let resp = client
        .get(url)
        .send()
        .await
        .unwrap_or_else(|e| panic!("GET {} failed: {}", url, e));
    assert!(
        resp.status().is_success(),
        "GET {} returned status {}",
        url,
        resp.status()
    );
    resp.json().await.unwrap_or_else(|e| {
        panic!("Failed to parse JSON from {}: {}", url, e);
    })
}
