use std::sync::Arc;
use tokio::task::AbortHandle;
use tokio_stream::wrappers::TcpListenerStream;
use tokio_util::sync::CancellationToken;
use tonic::transport::Server as TonicServer;
use tracing::{error, info};

use sofa_registry_core::pb::sofa::registry::meta::meta_service_server::MetaServiceServer;
use sofa_registry_core::slot::SlotConfig;
use sofa_registry_server_shared::metrics as srv_metrics;
use sofa_registry_store::traits::leader_elector::LeaderElector;

use crate::config::MetaServerConfig;
use crate::grpc::MetaGrpcServiceImpl;
use crate::http;
use crate::leader::MetaLeaderElector;
use crate::lease::{DataServerManager, SessionServerManager};
use crate::slot::MetaSlotManager;

/// Shared state accessible by HTTP handlers and gRPC service
pub struct MetaServerState {
    pub config: MetaServerConfig,
    pub leader_elector: Arc<MetaLeaderElector>,
    pub data_server_manager: Arc<DataServerManager>,
    pub session_server_manager: Arc<SessionServerManager>,
    pub slot_manager: Arc<MetaSlotManager>,
}

/// The Meta Server orchestrates cluster coordination:
/// - Leader election via distributed lock
/// - Slot table management and assignment
/// - Node lease management for data and session servers
/// - Admin HTTP API
/// - gRPC service for session/data server communication
pub struct MetaServer {
    config: MetaServerConfig,
    state: Arc<MetaServerState>,
    cancel: CancellationToken,
    server_abort_handles: Vec<AbortHandle>,
}

impl MetaServer {
    pub async fn new(
        config: MetaServerConfig,
        lock_repo: Arc<dyn sofa_registry_store::traits::distribute_lock::DistributeLockRepository>,
    ) -> Self {
        let data_server_manager = Arc::new(DataServerManager::new(config.data_lease_secs));
        let session_server_manager = Arc::new(SessionServerManager::new(config.session_lease_secs));

        let slot_config = SlotConfig {
            slot_num: config.slot_num,
            slot_replicas: config.slot_replicas,
            ..Default::default()
        };
        let slot_manager = Arc::new(MetaSlotManager::new(
            slot_config,
            data_server_manager.clone(),
        ));

        let leader_elector = Arc::new(MetaLeaderElector::new(
            lock_repo,
            config.grpc_address(),
            config.data_center.clone(),
            config.election_lock_duration_ms,
            config.election_interval_ms,
        ));

        let state = Arc::new(MetaServerState {
            config: config.clone(),
            leader_elector: leader_elector.clone(),
            data_server_manager,
            session_server_manager,
            slot_manager,
        });

        Self {
            config,
            state,
            cancel: CancellationToken::new(),
            server_abort_handles: Vec::new(),
        }
    }

    /// Start the meta server (gRPC + HTTP + election loop + eviction loop)
    pub async fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Starting Meta server...");

        // Start gRPC server -- pre-bind the listener so the port is released
        // promptly when the server is stopped (important for restart).
        let grpc_addr = format!("0.0.0.0:{}", self.config.grpc_port);
        let grpc_listener = tokio::net::TcpListener::bind(&grpc_addr).await?;
        let grpc_incoming = TcpListenerStream::new(grpc_listener);
        let grpc_service = MetaGrpcServiceImpl::new(self.state.clone());
        let cancel = self.cancel.clone();

        let grpc_handle = tokio::spawn(async move {
            info!("Meta gRPC server listening on {}", grpc_addr);
            let result = TonicServer::builder()
                .add_service(MetaServiceServer::new(grpc_service))
                .serve_with_incoming_shutdown(grpc_incoming, cancel.cancelled())
                .await;
            if let Err(e) = result {
                error!("Meta gRPC server error: {}", e);
            }
        });
        self.server_abort_handles.push(grpc_handle.abort_handle());

        // Start HTTP server
        let http_addr = format!("0.0.0.0:{}", self.config.http_port);
        let router = http::create_router(self.state.clone());
        let listener = tokio::net::TcpListener::bind(&http_addr).await?;
        let cancel = self.cancel.clone();

        let http_handle = tokio::spawn(async move {
            info!("Meta HTTP server listening on {}", http_addr);
            let result = axum::serve(listener, router)
                .with_graceful_shutdown(async move { cancel.cancelled().await })
                .await;
            if let Err(e) = result {
                error!("Meta HTTP server error: {}", e);
            }
        });
        self.server_abort_handles.push(http_handle.abort_handle());

        // Start leader election loop
        let elector = self.state.leader_elector.clone();
        let cancel = self.cancel.clone();
        tokio::spawn(async move {
            elector.run_election_loop(cancel).await;
        });

        // Start eviction + rebalance loop
        let state = self.state.clone();
        let eviction_interval = self.config.eviction_interval_secs;
        let cancel = self.cancel.clone();
        tokio::spawn(async move {
            let mut ticker =
                tokio::time::interval(tokio::time::Duration::from_secs(eviction_interval));
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    _ = ticker.tick() => {
                        // Set leader gauge
                        let is_leader = state.leader_elector.am_i_leader();
                        metrics::gauge!(srv_metrics::META_IS_LEADER).set(if is_leader { 1.0 } else { 0.0 });

                        // Evict expired leases
                        let evicted_data = state.data_server_manager.evict_expired();
                        let evicted_session = state.session_server_manager.evict_expired();

                        if !evicted_data.is_empty() || !evicted_session.is_empty() {
                            info!(
                                "Evicted {} data servers, {} session servers",
                                evicted_data.len(), evicted_session.len()
                            );
                            let total_evicted = evicted_data.len() + evicted_session.len();
                            metrics::counter!(srv_metrics::META_LEASE_EVICTIONS_TOTAL).increment(total_evicted as u64);
                        }

                        // Update server count gauges after eviction
                        metrics::gauge!(srv_metrics::META_DATA_SERVERS).set(state.data_server_manager.count() as f64);
                        metrics::gauge!(srv_metrics::META_SESSION_SERVERS).set(state.session_server_manager.count() as f64);

                        // Rebalance slots if needed
                        if is_leader && state.slot_manager.needs_rebalance() {
                            state.slot_manager.try_assign_or_rebalance();
                        }

                        // Update slot table epoch gauge after potential rebalance
                        metrics::gauge!(srv_metrics::META_SLOT_TABLE_EPOCH).set(state.slot_manager.get_epoch() as f64);
                    }
                }
            }
        });

        info!(
            "Meta server started (gRPC={}, HTTP={})",
            self.config.grpc_port, self.config.http_port
        );
        Ok(())
    }

    pub fn stop(&self) {
        info!("Stopping Meta server...");
        self.cancel.cancel();
    }

    /// Stop and wait for the gRPC/HTTP server tasks to finish, ensuring
    /// ports are released before returning.
    pub async fn stop_and_wait(&mut self) {
        info!("Stopping Meta server...");
        self.cancel.cancel();

        for handle in self.server_abort_handles.drain(..) {
            handle.abort();
        }

        tokio::task::yield_now().await;
    }

    pub fn state(&self) -> Arc<MetaServerState> {
        self.state.clone()
    }
}
