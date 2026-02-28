use clap::{Parser, Subcommand};
use serde::Deserialize;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(
    name = "sofa-registry",
    version,
    about = "SOFARegistry - High-performance service registry in Rust"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Config file path
    #[arg(short, long, default_value = "config.toml")]
    config: String,

    /// Log level
    #[arg(long, default_value = "info")]
    log_level: String,
}

#[derive(Subcommand)]
enum Commands {
    /// Run all servers in one process (development mode)
    All,
    /// Run session server only
    Session,
    /// Run data server only
    Data,
    /// Run meta server only
    Meta,
    /// Run MCP server for AI-assisted registry lookups
    Mcp,
}

/// Top-level configuration file matching config.example.toml.
#[derive(Debug, Deserialize)]
struct AppConfig {
    #[serde(default)]
    common: CommonSection,
    #[serde(default)]
    meta: MetaSection,
    #[serde(default)]
    data: DataSection,
    #[serde(default)]
    session: SessionSection,
    #[serde(default)]
    mcp: McpSection,
}

#[derive(Debug, Deserialize, Default)]
struct CommonSection {
    #[serde(default = "default_data_center")]
    data_center: String,
    #[serde(default = "default_cluster_id")]
    cluster_id: String,
    #[serde(default = "default_local_address")]
    local_address: String,
}

#[derive(Debug, Deserialize, Default)]
struct MetaSection {
    #[serde(default = "default_meta_grpc_port")]
    grpc_port: u16,
    #[serde(default = "default_meta_http_port")]
    http_port: u16,
    #[serde(default = "default_db_url")]
    db_url: String,
    #[serde(default = "default_meta_peers")]
    meta_peers: Vec<String>,
    #[serde(default = "default_session_lease_secs")]
    session_lease_secs: u64,
    #[serde(default = "default_data_lease_secs")]
    data_lease_secs: u64,
    #[serde(default = "default_slot_num")]
    slot_num: u32,
    #[serde(default = "default_slot_replicas")]
    slot_replicas: u32,
    #[serde(default = "default_election_lock_duration_ms")]
    election_lock_duration_ms: i64,
    #[serde(default = "default_election_interval_ms")]
    election_interval_ms: u64,
}

#[derive(Debug, Deserialize, Default)]
struct DataSection {
    #[serde(default = "default_data_grpc_port")]
    grpc_port: u16,
    #[serde(default = "default_data_http_port")]
    http_port: u16,
    #[serde(default = "default_meta_addresses")]
    meta_server_addresses: Vec<String>,
    #[serde(default = "default_slot_sync_interval")]
    slot_sync_interval_secs: u64,
    #[serde(default = "default_data_change_debounce_ms")]
    data_change_debounce_ms: u64,
    #[serde(default = "default_session_lease_secs")]
    session_lease_secs: u64,
}

#[derive(Debug, Deserialize, Default)]
struct SessionSection {
    #[serde(default = "default_session_grpc_port")]
    grpc_port: u16,
    #[serde(default = "default_session_http_port")]
    http_port: u16,
    #[serde(default = "default_meta_addresses")]
    meta_server_addresses: Vec<String>,
    #[serde(default = "default_push_task_timeout_ms")]
    push_task_timeout_ms: u64,
    #[serde(default = "default_push_task_buffer_size")]
    push_task_buffer_size: usize,
}

#[derive(Debug, Deserialize, Default)]
struct McpSection {
    #[serde(default = "default_meta_http_url")]
    meta_http_url: String,
    #[serde(default = "default_session_http_url")]
    session_http_url: String,
    #[serde(default = "default_data_http_url")]
    data_http_url: String,
}

// Default value functions
fn default_data_center() -> String {
    "DefaultDataCenter".into()
}
fn default_cluster_id() -> String {
    "DefaultCluster".into()
}
fn default_local_address() -> String {
    detect_local_ip()
}
fn default_meta_grpc_port() -> u16 {
    9611
}
fn default_meta_http_port() -> u16 {
    9612
}
fn default_db_url() -> String {
    "sqlite://sofa-registry-meta.db?mode=rwc".into()
}
fn default_meta_peers() -> Vec<String> {
    vec!["127.0.0.1:9611".into()]
}
fn default_session_lease_secs() -> u64 {
    30
}
fn default_data_lease_secs() -> u64 {
    30
}
fn default_slot_num() -> u32 {
    256
}
fn default_slot_replicas() -> u32 {
    2
}
fn default_election_lock_duration_ms() -> i64 {
    30000
}
fn default_election_interval_ms() -> u64 {
    1000
}
fn default_data_grpc_port() -> u16 {
    9621
}
fn default_data_http_port() -> u16 {
    9622
}
fn default_meta_addresses() -> Vec<String> {
    vec!["127.0.0.1:9611".into()]
}
fn default_slot_sync_interval() -> u64 {
    6
}
fn default_data_change_debounce_ms() -> u64 {
    500
}
fn default_session_grpc_port() -> u16 {
    9601
}
fn default_session_http_port() -> u16 {
    9602
}
fn default_push_task_timeout_ms() -> u64 {
    3000
}
fn default_push_task_buffer_size() -> usize {
    10000
}
fn default_meta_http_url() -> String {
    "http://127.0.0.1:9612".into()
}
fn default_session_http_url() -> String {
    "http://127.0.0.1:9602".into()
}
fn default_data_http_url() -> String {
    "http://127.0.0.1:9622".into()
}

/// Detect the local machine's IP by connecting a UDP socket.
fn detect_local_ip() -> String {
    use std::net::UdpSocket;
    let socket = match UdpSocket::bind("0.0.0.0:0") {
        Ok(s) => s,
        Err(_) => return "127.0.0.1".to_string(),
    };
    if socket.connect("8.8.8.8:80").is_err() {
        return "127.0.0.1".to_string();
    }
    socket
        .local_addr()
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|_| "127.0.0.1".to_string())
}

fn load_config(path: &str) -> anyhow::Result<AppConfig> {
    match std::fs::read_to_string(path) {
        Ok(contents) => {
            let config: AppConfig = toml::from_str(&contents)?;
            info!("Loaded configuration from {}", path);
            Ok(config)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            info!("Config file {} not found, using defaults", path);
            Ok(AppConfig {
                common: CommonSection::default(),
                meta: MetaSection::default(),
                data: DataSection::default(),
                session: SessionSection::default(),
                mcp: McpSection::default(),
            })
        }
        Err(e) => Err(anyhow::anyhow!(
            "Failed to read config file {}: {}",
            path,
            e
        )),
    }
}

impl AppConfig {
    fn to_meta_config(&self) -> sofa_registry_server_meta::config::MetaServerConfig {
        sofa_registry_server_meta::config::MetaServerConfig {
            data_center: self.common.data_center.clone(),
            cluster_id: self.common.cluster_id.clone(),
            local_address: self.common.local_address.clone(),
            grpc_port: self.meta.grpc_port,
            http_port: self.meta.http_port,
            db_url: self.meta.db_url.clone(),
            meta_peers: self.meta.meta_peers.clone(),
            session_lease_secs: self.meta.session_lease_secs,
            data_lease_secs: self.meta.data_lease_secs,
            slot_num: self.meta.slot_num,
            slot_replicas: self.meta.slot_replicas,
            election_lock_duration_ms: self.meta.election_lock_duration_ms,
            election_interval_ms: self.meta.election_interval_ms,
            eviction_interval_secs: 5,
        }
    }

    fn to_data_config(&self) -> sofa_registry_server_data::config::DataServerConfig {
        sofa_registry_server_data::config::DataServerConfig {
            data_center: self.common.data_center.clone(),
            cluster_id: self.common.cluster_id.clone(),
            local_address: self.common.local_address.clone(),
            grpc_port: self.data.grpc_port,
            http_port: self.data.http_port,
            meta_server_addresses: self.data.meta_server_addresses.clone(),
            slot_sync_interval_secs: self.data.slot_sync_interval_secs,
            data_change_debounce_ms: self.data.data_change_debounce_ms,
            session_lease_secs: self.data.session_lease_secs,
            slot_num: self.meta.slot_num,
        }
    }

    fn to_session_config(&self) -> sofa_registry_server_session::config::SessionServerConfig {
        sofa_registry_server_session::config::SessionServerConfig {
            data_center: self.common.data_center.clone(),
            cluster_id: self.common.cluster_id.clone(),
            local_address: self.common.local_address.clone(),
            grpc_port: self.session.grpc_port,
            http_port: self.session.http_port,
            meta_server_addresses: self.session.meta_server_addresses.clone(),
            push_task_timeout_ms: self.session.push_task_timeout_ms,
            push_task_buffer_size: self.session.push_task_buffer_size,
            slot_num: self.meta.slot_num,
            connection_idle_timeout_secs: 90,
        }
    }

    fn to_mcp_config(&self) -> sofa_registry_mcp::server::McpConfig {
        sofa_registry_mcp::server::McpConfig {
            meta_http_url: self.mcp.meta_http_url.clone(),
            session_http_url: self.mcp.session_http_url.clone(),
            data_http_url: self.mcp.data_http_url.clone(),
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&cli.log_level)),
        )
        .with_target(true)
        .with_thread_ids(true)
        .init();

    info!("SOFARegistry-RS v{}", env!("CARGO_PKG_VERSION"));

    // Install Prometheus metrics recorder (global, once per process)
    sofa_registry_server_shared::metrics::install_metrics_recorder();
    info!("Prometheus metrics recorder installed");

    let app_config = load_config(&cli.config)?;

    match cli.command {
        Commands::All => {
            info!("Starting all servers in development mode...");
            run_all(app_config).await?;
        }
        Commands::Meta => {
            info!("Starting Meta server...");
            run_meta(app_config.to_meta_config()).await?;
        }
        Commands::Data => {
            info!("Starting Data server...");
            run_data(app_config.to_data_config()).await?;
        }
        Commands::Session => {
            info!("Starting Session server...");
            run_session(app_config.to_session_config()).await?;
        }
        Commands::Mcp => {
            info!("Starting MCP server...");
            run_mcp(app_config.to_mcp_config()).await?;
        }
    }

    Ok(())
}

async fn run_meta(
    config: sofa_registry_server_meta::config::MetaServerConfig,
) -> anyhow::Result<()> {
    let pool = sofa_registry_store::jdbc::create_pool(&config.db_url)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create DB pool: {}", e))?;

    sofa_registry_store::jdbc::run_migrations(&pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to run migrations: {}", e))?;

    let lock_repo = std::sync::Arc::new(sofa_registry_store::jdbc::SqliteDistributeLockRepo::new(
        pool,
    ));

    let server = sofa_registry_server_meta::server::MetaServer::new(config, lock_repo).await;
    server.start().await.map_err(|e| anyhow::anyhow!("{}", e))?;

    tokio::signal::ctrl_c().await?;
    server.stop();
    info!("Meta server stopped");
    Ok(())
}

async fn run_data(
    config: sofa_registry_server_data::config::DataServerConfig,
) -> anyhow::Result<()> {
    let mut server = sofa_registry_server_data::server::DataServer::new(config);
    server.start().await.map_err(|e| anyhow::anyhow!("{}", e))?;

    tokio::signal::ctrl_c().await?;
    server.shutdown();
    info!("Data server stopped");
    Ok(())
}

async fn run_session(
    config: sofa_registry_server_session::config::SessionServerConfig,
) -> anyhow::Result<()> {
    let mut server = sofa_registry_server_session::server::SessionServer::new(config);
    server.start().await.map_err(|e| anyhow::anyhow!("{}", e))?;

    tokio::signal::ctrl_c().await?;
    server.stop();
    info!("Session server stopped");
    Ok(())
}

async fn run_mcp(config: sofa_registry_mcp::server::McpConfig) -> anyhow::Result<()> {
    let server = sofa_registry_mcp::server::McpServer::new(config)?;
    server
        .run_stdio()
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))
}

async fn run_all(app_config: AppConfig) -> anyhow::Result<()> {
    info!("Starting all servers...");

    let meta_config = app_config.to_meta_config();
    let data_config = app_config.to_data_config();
    let session_config = app_config.to_session_config();

    // Start meta first
    let meta_db_url = meta_config.db_url.clone();
    let pool = sofa_registry_store::jdbc::create_pool(&meta_db_url)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create DB pool: {}", e))?;
    sofa_registry_store::jdbc::run_migrations(&pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to run migrations: {}", e))?;
    let lock_repo = std::sync::Arc::new(sofa_registry_store::jdbc::SqliteDistributeLockRepo::new(
        pool,
    ));

    let meta_server =
        sofa_registry_server_meta::server::MetaServer::new(meta_config, lock_repo).await;
    meta_server
        .start()
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    info!("Meta server started");

    // Give meta time to elect leader
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Start data
    let mut data_server = sofa_registry_server_data::server::DataServer::new(data_config);
    data_server
        .start()
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    info!("Data server started");

    // Give data time to register with meta
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // Start session
    let mut session_server =
        sofa_registry_server_session::server::SessionServer::new(session_config);
    session_server
        .start()
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    info!("Session server started");

    info!("All servers started. Press Ctrl-C to stop.");
    tokio::signal::ctrl_c().await?;
    info!("Shutting down all servers...");

    // Graceful shutdown in reverse order
    session_server.stop();
    info!("Session server stopped");
    data_server.shutdown();
    info!("Data server stopped");
    meta_server.stop();
    info!("Meta server stopped");

    Ok(())
}
