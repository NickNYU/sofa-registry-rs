use std::sync::Arc;

use tokio::task::AbortHandle;
use tokio_stream::wrappers::TcpListenerStream;
use tokio_util::sync::CancellationToken;
use tonic::transport::Server as TonicServer;
use tracing::{error, info};

use sofa_registry_core::pb::sofa::registry::session::session_service_server::SessionServiceServer;
use sofa_registry_remoting::GrpcClientPool;

use crate::cache::SessionCacheService;
use crate::config::SessionServerConfig;
use crate::connection::ConnectionService;
use crate::http;
use crate::push::{PushReceiver, PushService, StreamRegistry};
use crate::registry::{PublisherRegistry, SubscriberRegistry};
use crate::remoting::SessionGrpcServiceImpl;
use crate::slot::SessionSlotManager;
use crate::write::{WriteDataAcceptor, WriteDataReceiver};
use sofa_registry_server_shared::meta_client::MetaClient;
use sofa_registry_store::traits::MetaServiceClient as _;

/// Shared state accessible by HTTP handlers and gRPC service.
pub struct SessionServerState {
    pub config: SessionServerConfig,
    pub publisher_registry: PublisherRegistry,
    pub subscriber_registry: SubscriberRegistry,
    pub connection_service: ConnectionService,
    pub cache_service: SessionCacheService,
    pub push_service: PushService,
    pub write_acceptor: WriteDataAcceptor,
    pub stream_registry: Arc<StreamRegistry>,
    pub slot_manager: Arc<SessionSlotManager>,
    pub data_client_pool: Arc<GrpcClientPool>,
}

/// The Session Server:
/// - Accepts client publisher and subscriber registrations via gRPC
/// - Pushes data to subscribers when data changes
/// - Forwards writes to the Data server
/// - Caches data versions from Data server
/// - Registers with Meta server for slot table
pub struct SessionServer {
    config: SessionServerConfig,
    state: Arc<SessionServerState>,
    cancel: CancellationToken,
    push_receiver: Option<PushReceiver>,
    write_receiver: Option<WriteDataReceiver>,
    server_abort_handles: Vec<AbortHandle>,
}

impl SessionServer {
    pub fn new(config: SessionServerConfig) -> Self {
        let stream_registry = Arc::new(StreamRegistry::new());
        let (push_service, push_receiver) =
            PushService::new(config.push_task_buffer_size, stream_registry.clone());

        let slot_manager = Arc::new(SessionSlotManager::new(config.slot_num));
        let grpc_pool = Arc::new(GrpcClientPool::new());
        let data_client_pool = Arc::new(GrpcClientPool::new());
        let (write_acceptor, write_receiver) = WriteDataAcceptor::new(
            4096,
            grpc_pool,
            slot_manager.clone(),
            config.data_center.clone(),
            config.grpc_address(),
        );

        let state = Arc::new(SessionServerState {
            config: config.clone(),
            publisher_registry: PublisherRegistry::new(),
            subscriber_registry: SubscriberRegistry::new(),
            connection_service: ConnectionService::new(),
            cache_service: SessionCacheService::new(),
            push_service,
            write_acceptor,
            stream_registry,
            slot_manager,
            data_client_pool,
        });

        Self {
            config,
            state,
            cancel: CancellationToken::new(),
            push_receiver: Some(push_receiver),
            write_receiver: Some(write_receiver),
            server_abort_handles: Vec::new(),
        }
    }

    /// Start the session server (gRPC + HTTP + background tasks).
    pub async fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Starting Session server...");

        // Start gRPC server -- pre-bind the listener so the port is released
        // promptly when the server is stopped (important for restart).
        let grpc_addr = format!("0.0.0.0:{}", self.config.grpc_port);
        let grpc_listener = tokio::net::TcpListener::bind(&grpc_addr).await?;
        let grpc_incoming = TcpListenerStream::new(grpc_listener);
        let grpc_service = SessionGrpcServiceImpl::new(self.state.clone());
        let cancel = self.cancel.clone();

        let grpc_handle = tokio::spawn(async move {
            info!("Session gRPC server listening on {}", grpc_addr);
            let result = TonicServer::builder()
                .add_service(SessionServiceServer::new(grpc_service))
                .serve_with_incoming_shutdown(grpc_incoming, cancel.cancelled())
                .await;
            if let Err(e) = result {
                error!("Session gRPC server error: {}", e);
            }
        });
        self.server_abort_handles.push(grpc_handle.abort_handle());

        // Start HTTP server
        let http_addr = format!("0.0.0.0:{}", self.config.http_port);
        let router = http::create_router(self.state.clone());
        let listener = tokio::net::TcpListener::bind(&http_addr).await?;
        let cancel = self.cancel.clone();

        let http_handle = tokio::spawn(async move {
            info!("Session HTTP server listening on {}", http_addr);
            let result = axum::serve(listener, router)
                .with_graceful_shutdown(async move { cancel.cancelled().await })
                .await;
            if let Err(e) = result {
                error!("Session HTTP server error: {}", e);
            }
        });
        self.server_abort_handles.push(http_handle.abort_handle());

        // Start push receiver
        if let Some(push_receiver) = self.push_receiver.take() {
            let cancel = self.cancel.clone();
            tokio::spawn(async move {
                push_receiver.run(cancel).await;
            });
        }

        // Start write data receiver
        if let Some(write_receiver) = self.write_receiver.take() {
            let cancel = self.cancel.clone();
            tokio::spawn(async move {
                write_receiver.run(cancel).await;
            });
        }

        // Register with meta server and start heartbeat loop
        let meta_client = MetaClient::for_session(
            self.config.meta_server_addresses.clone(),
            self.config.grpc_address(),
            self.config.data_center.clone(),
            self.config.cluster_id.clone(),
        );

        match meta_client.register_node().await {
            Ok(Some(slot_table)) => {
                info!(
                    "Registered with meta server, slot table epoch={}",
                    slot_table.epoch
                );
            }
            Ok(None) => {
                info!("Registered with meta server, no slot table yet");
            }
            Err(e) => {
                error!(
                    "Failed to register with meta server: {}. Will retry via heartbeat.",
                    e
                );
            }
        }

        let cancel = self.cancel.clone();
        tokio::spawn(async move {
            meta_client.run_heartbeat_loop(10, 30, cancel).await;
        });

        // Start slot table sync loop
        let slot_mgr = self.state.slot_manager.clone();
        let meta_client_for_slot = MetaClient::for_session(
            self.config.meta_server_addresses.clone(),
            self.config.grpc_address(),
            self.config.data_center.clone(),
            self.config.cluster_id.clone(),
        );
        let cancel = self.cancel.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(6));
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    _ = interval.tick() => {
                        let epoch = slot_mgr.get_epoch();
                        if let Some(table) = meta_client_for_slot.fetch_domain_slot_table(epoch).await {
                            if table.epoch > epoch {
                                info!("Session slot table updated: epoch {} -> {}", epoch, table.epoch);
                                slot_mgr.update_slot_table(table);
                            }
                        }
                    }
                }
            }
        });

        // Start connection eviction loop
        let state_for_eviction = self.state.clone();
        let idle_timeout = self.config.connection_idle_timeout_secs;
        let cancel = self.cancel.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(15));
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    _ = interval.tick() => {
                        let evicted = state_for_eviction.connection_service.evict_idle(idle_timeout);
                        for client_id in &evicted {
                            // Clean up the subscribe stream for evicted clients
                            state_for_eviction.stream_registry.unregister(client_id);
                            info!("Evicted idle client connection: {}", client_id);
                        }
                    }
                }
            }
        });

        info!(
            "Session server started (gRPC={}, HTTP={})",
            self.config.grpc_port, self.config.http_port
        );
        Ok(())
    }

    pub fn stop(&self) {
        info!("Stopping Session server...");
        self.cancel.cancel();
    }

    /// Stop and wait for the gRPC/HTTP server tasks to finish, ensuring
    /// ports are released before returning. Aborts server tasks to guarantee
    /// the listeners are dropped and ports freed.
    pub async fn stop_and_wait(&mut self) {
        info!("Stopping Session server...");
        self.cancel.cancel();

        // Abort the server tasks to ensure the pre-bound listeners are
        // dropped immediately, freeing the ports for a restart.
        for handle in self.server_abort_handles.drain(..) {
            handle.abort();
        }

        // Brief yield to let the runtime process the abort.
        tokio::task::yield_now().await;
    }

    pub fn state(&self) -> Arc<SessionServerState> {
        self.state.clone()
    }
}
