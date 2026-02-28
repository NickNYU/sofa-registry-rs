use std::sync::Arc;

use sofa_registry_core::pb::sofa::registry::data::data_service_server::DataServiceServer;
use sofa_registry_remoting::{AxumHttpServer, GrpcServer};
use sofa_registry_server_shared::metrics as srv_metrics;
use sofa_registry_store::memory::LocalDatumStorage;
use sofa_registry_store::traits::DatumStorage;
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::change::{DataChangeEventCenter, DataChangeReceiver};
use crate::config::DataServerConfig;
use crate::http::routes::create_router;
use crate::lease::SessionLeaseManager;
use crate::remoting::data_grpc_service::DataGrpcService;
use crate::replication::SlotDiffSyncer;
use crate::slot::DataSlotManager;
use sofa_registry_server_shared::meta_client::MetaClient;
use sofa_registry_store::traits::MetaServiceClient as _;

/// Shared state accessible from HTTP handlers and gRPC service.
pub struct DataServerState {
    pub config: DataServerConfig,
    pub storage: Arc<dyn DatumStorage>,
    pub slot_manager: Arc<DataSlotManager>,
    pub session_lease_manager: Arc<SessionLeaseManager>,
    pub change_center: DataChangeEventCenter,
    pub start_time: i64,
}

/// The main Data Server that orchestrates all components.
pub struct DataServer {
    config: DataServerConfig,
    storage: Arc<dyn DatumStorage>,
    slot_manager: Arc<DataSlotManager>,
    session_lease_manager: Arc<SessionLeaseManager>,
    change_center: DataChangeEventCenter,
    change_receiver: Option<DataChangeReceiver>,
    meta_client: Arc<MetaClient>,
    diff_syncer: Arc<SlotDiffSyncer>,
    cancel: CancellationToken,
    start_time: i64,
    // Keep server handles alive so their shutdown channels are not dropped.
    _grpc_server: Option<GrpcServer>,
    _http_server: Option<AxumHttpServer>,
}

impl DataServer {
    pub fn new(config: DataServerConfig) -> Self {
        let storage: Arc<dyn DatumStorage> = Arc::new(LocalDatumStorage::new(config.slot_num));
        let slot_manager = Arc::new(DataSlotManager::new(&config.grpc_address()));
        let session_lease_manager = Arc::new(SessionLeaseManager::new(config.session_lease_secs));
        let notify_pool = Arc::new(sofa_registry_remoting::GrpcClientPool::new());
        let (change_center, change_receiver) = DataChangeEventCenter::new(
            4096,
            notify_pool,
            session_lease_manager.clone(),
            config.grpc_address(),
        );
        let meta_client = Arc::new(MetaClient::for_data(
            config.meta_server_addresses.clone(),
            config.grpc_address(),
            config.data_center.clone(),
            config.cluster_id.clone(),
        ));
        let diff_syncer = Arc::new(SlotDiffSyncer::new(storage.clone(), config.slot_num));
        let cancel = CancellationToken::new();

        Self {
            config,
            storage,
            slot_manager,
            session_lease_manager,
            change_center,
            change_receiver: Some(change_receiver),
            meta_client,
            diff_syncer,
            cancel,
            start_time: chrono::Utc::now().timestamp_millis(),
            _grpc_server: None,
            _http_server: None,
        }
    }

    /// Start all data server components.
    pub async fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!(
            "Starting Data Server on grpc={} http={}",
            self.config.grpc_address(),
            self.config.http_address()
        );

        // Register with Meta server.
        self.register_with_meta().await;

        // Start the gRPC server.
        self.start_grpc_server().await?;

        // Start the HTTP admin server.
        self.start_http_server().await?;

        // Start the change event merge loop.
        self.start_change_loop();

        // Start the slot sync loop.
        self.start_slot_sync_loop();

        // Start the session lease eviction loop.
        self.start_lease_eviction_loop();

        info!("Data Server started successfully");
        Ok(())
    }

    async fn register_with_meta(&self) {
        info!("Registering with Meta server...");
        match self.meta_client.register_node().await {
            Ok(Some(slot_table)) => {
                info!(
                    "Received slot table from Meta (epoch={}, {} slots)",
                    slot_table.epoch,
                    slot_table.slot_count()
                );
                self.slot_manager.update_slot_table(slot_table);
            }
            Ok(None) => {
                info!("Registered with Meta but no slot table received yet");
            }
            Err(e) => {
                info!(
                    "Failed to register with Meta: {}; will retry in sync loop",
                    e
                );
            }
        }
    }

    async fn start_grpc_server(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let service = DataGrpcService::new(
            self.storage.clone(),
            self.slot_manager.clone(),
            self.change_center.clone(),
            self.diff_syncer.clone(),
            self.config.data_center.clone(),
            self.session_lease_manager.clone(),
        );

        let router =
            tonic::transport::Server::builder().add_service(DataServiceServer::new(service));

        let mut grpc_server = GrpcServer::new(self.config.grpc_port);
        grpc_server.start(router).await?;
        info!("Data gRPC server started on port {}", self.config.grpc_port);
        self._grpc_server = Some(grpc_server);
        Ok(())
    }

    async fn start_http_server(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let state = Arc::new(DataServerState {
            config: self.config.clone(),
            storage: self.storage.clone(),
            slot_manager: self.slot_manager.clone(),
            session_lease_manager: self.session_lease_manager.clone(),
            change_center: self.change_center.clone(),
            start_time: self.start_time,
        });

        let router = create_router(state);
        let mut http_server = AxumHttpServer::new(self.config.http_port);
        http_server.start(router).await?;
        info!("Data HTTP server started on port {}", self.config.http_port);
        self._http_server = Some(http_server);
        Ok(())
    }

    fn start_change_loop(&mut self) {
        if let Some(receiver) = self.change_receiver.take() {
            let debounce_ms = self.config.data_change_debounce_ms;
            let cancel = self.cancel.clone();
            tokio::spawn(async move {
                receiver.run_merge_loop(debounce_ms, cancel).await;
            });
            info!("Data change merge loop started");
        }
    }

    fn start_slot_sync_loop(&self) {
        let meta_client = self.meta_client.clone();
        let slot_manager = self.slot_manager.clone();
        let interval_secs = self.config.slot_sync_interval_secs;
        let cancel = self.cancel.clone();

        tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(tokio::time::Duration::from_secs(interval_secs));
            loop {
                tokio::select! {
                    biased;
                    _ = cancel.cancelled() => {
                        info!("Slot sync loop cancelled");
                        return;
                    }
                    _ = interval.tick() => {
                        let current_epoch = slot_manager.get_slot_table_epoch();
                        if let Some(new_table) = meta_client.fetch_domain_slot_table(current_epoch).await {
                            if new_table.epoch > current_epoch {
                                info!(
                                    "Updated slot table: epoch {} -> {}",
                                    current_epoch, new_table.epoch
                                );
                                slot_manager.update_slot_table(new_table);
                            }
                        }

                        metrics::gauge!(srv_metrics::DATA_SLOT_TABLE_EPOCH).set(slot_manager.get_slot_table_epoch() as f64);

                        // Also renew our lease.
                        let _ = meta_client.renew_node(30).await;
                    }
                }
            }
        });
        info!("Slot sync loop started (interval={}s)", interval_secs);
    }

    fn start_lease_eviction_loop(&self) {
        let lease_manager = self.session_lease_manager.clone();
        let storage = self.storage.clone();
        let data_center = self.config.data_center.clone();
        let change_center = self.change_center.clone();
        let cancel = self.cancel.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(10));
            loop {
                tokio::select! {
                    biased;
                    _ = cancel.cancelled() => {
                        info!("Lease eviction loop cancelled");
                        return;
                    }
                    _ = interval.tick() => {
                        let expired = lease_manager.evict_expired();
                        for session_addr in expired {
                            info!("Cleaning up publishers for expired session: {}", session_addr);
                            // Parse session address into a ProcessId for cleanup.
                            let pid = sofa_registry_core::model::ProcessId::new(&session_addr, 0, 0);
                            let updated = storage.remove_publishers_by_session(&data_center, &pid);
                            for (data_info_id, version) in updated {
                                change_center.on_change(crate::change::DataChangeEvent {
                                    data_center: data_center.clone(),
                                    data_info_id,
                                    version,
                                });
                            }
                        }

                        let active_count = lease_manager.active_sessions().len();
                        metrics::gauge!(srv_metrics::DATA_ACTIVE_SESSION_LEASES).set(active_count as f64);
                    }
                }
            }
        });
    }

    /// Shut down all background tasks.
    pub fn shutdown(&self) {
        info!("Shutting down Data Server");
        self.cancel.cancel();
    }

    /// Wait until cancelled.
    pub async fn wait_for_shutdown(&self) {
        self.cancel.cancelled().await;
    }

    pub fn config(&self) -> &DataServerConfig {
        &self.config
    }

    pub fn storage(&self) -> &Arc<dyn DatumStorage> {
        &self.storage
    }

    pub fn slot_manager(&self) -> &Arc<DataSlotManager> {
        &self.slot_manager
    }
}
