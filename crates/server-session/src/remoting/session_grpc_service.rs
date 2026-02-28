use std::pin::Pin;
use std::sync::Arc;

use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};
use tracing::{debug, info, warn};

use sofa_registry_server_shared::metrics as srv_metrics;

use sofa_registry_core::model::{
    ConnectId, ProcessId, Publisher, RegisterVersion, Subscriber,
};
use sofa_registry_core::model::publish_type::{PublishSource, PublishType};
use sofa_registry_core::model::Scope;
use sofa_registry_core::pb::sofa::registry::{
    PublisherRegisterPb, RegisterResponsePb, SubscriberRegisterPb, ReceivedDataPb,
};
use sofa_registry_core::pb::sofa::registry::session::{
    session_service_server::SessionService,
    ClientHeartbeatRequest, ClientHeartbeatResponse,
    DataChangeNotification, DataChangeNotificationResponse,
    SubscribeStreamRequest, UnregisterRequest,
};

use crate::server::SessionServerState;
use crate::write::WriteRequest;

pub struct SessionGrpcServiceImpl {
    state: Arc<SessionServerState>,
}

impl SessionGrpcServiceImpl {
    pub fn new(state: Arc<SessionServerState>) -> Self {
        Self { state }
    }
}

/// Convert a `PublisherRegisterPb` to a domain `Publisher`.
fn pb_to_publisher(pb: &PublisherRegisterPb, server_address: &str) -> Publisher {
    let base = pb.base.as_ref();

    let (data_info_id, data_id, instance_id, group, regist_id, client_id, ip, port, version, timestamp) =
        match base {
            Some(b) => (
                b.data_info_id.clone(),
                b.data_id.clone(),
                b.instance_id.clone(),
                b.group.clone(),
                b.regist_id.clone(),
                b.client_id.clone(),
                b.ip.clone(),
                b.port,
                b.version,
                b.timestamp,
            ),
            None => (
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                0,
                0,
                0,
            ),
        };

    let process_id = base
        .map(|b| ProcessId::new(&b.process_id, 0, 0))
        .unwrap_or_else(|| ProcessId::new("", 0, 0));

    let source_address = ConnectId::new(&ip, port as u16, server_address, 0);

    let attributes = base
        .map(|b| b.attributes.clone())
        .unwrap_or_default();

    Publisher {
        data_info_id,
        data_id,
        instance_id,
        group,
        regist_id,
        client_id,
        cell: None,
        app_name: base.and_then(|b| {
            if b.app_name.is_empty() {
                None
            } else {
                Some(b.app_name.clone())
            }
        }),
        process_id,
        version: RegisterVersion::new(version, timestamp),
        source_address,
        session_process_id: ProcessId::new(server_address, 0, 0),
        data_list: Vec::new(),
        publish_type: PublishType::Normal,
        publish_source: PublishSource::Client,
        attributes,
        register_timestamp: chrono::Utc::now().timestamp_millis(),
    }
}

/// Convert a `SubscriberRegisterPb` to a domain `Subscriber`.
fn pb_to_subscriber(pb: &SubscriberRegisterPb, server_address: &str) -> Subscriber {
    let base = pb.base.as_ref();

    let (data_info_id, data_id, instance_id, group, regist_id, client_id, ip, port) = match base {
        Some(b) => (
            b.data_info_id.clone(),
            b.data_id.clone(),
            b.instance_id.clone(),
            b.group.clone(),
            b.regist_id.clone(),
            b.client_id.clone(),
            b.ip.clone(),
            b.port,
        ),
        None => (
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            0,
        ),
    };

    let process_id = base
        .map(|b| ProcessId::new(&b.process_id, 0, 0))
        .unwrap_or_else(|| ProcessId::new("", 0, 0));

    let source_address = ConnectId::new(&ip, port as u16, server_address, 0);

    let scope = match pb.scope.as_str() {
        sofa_registry_core::constants::scope::ZONE => Scope::Zone,
        sofa_registry_core::constants::scope::GLOBAL => Scope::Global,
        _ => Scope::DataCenter,
    };

    Subscriber {
        data_info_id,
        data_id,
        instance_id,
        group,
        regist_id,
        client_id,
        scope,
        cell: None,
        app_name: base.and_then(|b| {
            if b.app_name.is_empty() {
                None
            } else {
                Some(b.app_name.clone())
            }
        }),
        process_id,
        source_address,
        accept_encoding: if pb.accept_encoding.is_empty() {
            None
        } else {
            Some(pb.accept_encoding.clone())
        },
        accept_multi: pb.accept_multi,
        register_timestamp: chrono::Utc::now().timestamp_millis(),
    }
}

#[tonic::async_trait]
impl SessionService for SessionGrpcServiceImpl {
    async fn register_publisher(
        &self,
        request: Request<PublisherRegisterPb>,
    ) -> Result<Response<RegisterResponsePb>, Status> {
        let pb = request.into_inner();
        let publisher = pb_to_publisher(&pb, &self.state.config.local_address);
        let regist_id = publisher.regist_id.clone();
        let data_info_id = publisher.data_info_id.clone();
        let client_id = publisher.client_id.clone();

        info!(
            "RegisterPublisher: data_info_id={}, regist_id={}, client_id={}",
            data_info_id, regist_id, client_id
        );

        // Track connection
        self.state.connection_service.connect(
            client_id.clone(),
            publisher.source_address.to_string(),
        );

        // Forward write to data server via the write acceptor
        self.state
            .write_acceptor
            .accept(WriteRequest::Publish {
                publisher: Box::new(publisher.clone()),
            })
            .await;

        // Register locally
        self.state.publisher_registry.register(publisher);

        // Metrics
        metrics::counter!(srv_metrics::GRPC_REQUESTS_TOTAL, "method" => "register_publisher").increment(1);
        metrics::gauge!(srv_metrics::SESSION_ACTIVE_PUBLISHERS)
            .set(self.state.publisher_registry.count() as f64);

        Ok(Response::new(RegisterResponsePb {
            success: true,
            regist_id,
            version: chrono::Utc::now().timestamp_millis(),
            refused: false,
            message: String::new(),
        }))
    }

    async fn register_subscriber(
        &self,
        request: Request<SubscriberRegisterPb>,
    ) -> Result<Response<RegisterResponsePb>, Status> {
        let pb = request.into_inner();
        let subscriber = pb_to_subscriber(&pb, &self.state.config.local_address);
        let regist_id = subscriber.regist_id.clone();
        let data_info_id = subscriber.data_info_id.clone();
        let client_id = subscriber.client_id.clone();

        info!(
            "RegisterSubscriber: data_info_id={}, regist_id={}, client_id={}",
            data_info_id, regist_id, client_id
        );

        // Track connection
        self.state.connection_service.connect(
            client_id.clone(),
            subscriber.source_address.to_string(),
        );

        // Register locally
        self.state.subscriber_registry.register(subscriber);

        // Metrics
        metrics::counter!(srv_metrics::GRPC_REQUESTS_TOTAL, "method" => "register_subscriber").increment(1);
        metrics::gauge!(srv_metrics::SESSION_ACTIVE_SUBSCRIBERS)
            .set(self.state.subscriber_registry.count() as f64);

        // Check if we have cached data to push immediately
        if let Some(version) = self.state.cache_service.get_version(&data_info_id) {
            debug!(
                "Subscriber {} has cached version {} for {}",
                regist_id, version.value, data_info_id
            );
        }

        Ok(Response::new(RegisterResponsePb {
            success: true,
            regist_id,
            version: chrono::Utc::now().timestamp_millis(),
            refused: false,
            message: String::new(),
        }))
    }

    async fn unregister(
        &self,
        request: Request<UnregisterRequest>,
    ) -> Result<Response<RegisterResponsePb>, Status> {
        let req = request.into_inner();
        let registry_type = req.registry_type.clone();

        let base = req.base.ok_or_else(|| {
            Status::invalid_argument("Missing base register info")
        })?;

        let data_info_id = &base.data_info_id;
        let regist_id = &base.regist_id;

        info!(
            "Unregister: type={}, data_info_id={}, regist_id={}",
            registry_type, data_info_id, regist_id
        );

        metrics::counter!(srv_metrics::GRPC_REQUESTS_TOTAL, "method" => "unregister").increment(1);

        match registry_type.as_str() {
            sofa_registry_core::constants::value_constants::PUBLISH => {
                if let Some(publisher) =
                    self.state.publisher_registry.unregister(data_info_id, regist_id)
                {
                    self.state
                        .write_acceptor
                        .accept(WriteRequest::Unpublish {
                            data_info_id: publisher.data_info_id,
                            regist_id: publisher.regist_id,
                        })
                        .await;
                }
            }
            sofa_registry_core::constants::value_constants::SUBSCRIBE => {
                self.state
                    .subscriber_registry
                    .unregister(data_info_id, regist_id);
            }
            other => {
                warn!("Unknown unregister type: {}", other);
                return Err(Status::invalid_argument(format!(
                    "Unknown registry type: {}",
                    other
                )));
            }
        }

        Ok(Response::new(RegisterResponsePb {
            success: true,
            regist_id: regist_id.to_string(),
            version: chrono::Utc::now().timestamp_millis(),
            refused: false,
            message: String::new(),
        }))
    }

    type SubscribeStream =
        Pin<Box<dyn tokio_stream::Stream<Item = Result<ReceivedDataPb, Status>> + Send>>;

    async fn subscribe(
        &self,
        request: Request<SubscribeStreamRequest>,
    ) -> Result<Response<Self::SubscribeStream>, Status> {
        let req = request.into_inner();
        info!(
            "Subscribe stream opened: client_id={}, zone={}, data_center={}",
            req.client_id, req.zone, req.data_center
        );

        // Create a channel for streaming push notifications to this client.
        // The PushReceiver will route tasks to per-client channels via the StreamRegistry.
        let (tx, rx) = mpsc::channel::<Result<ReceivedDataPb, Status>>(64);

        // Register this client's stream so PushReceiver can send data to it.
        self.state.stream_registry.register(&req.client_id, tx);

        // Metrics
        metrics::counter!(srv_metrics::GRPC_REQUESTS_TOTAL, "method" => "subscribe").increment(1);
        metrics::gauge!(srv_metrics::SESSION_ACTIVE_STREAMS)
            .set(self.state.stream_registry.count() as f64);

        let stream = ReceiverStream::new(rx);
        Ok(Response::new(Box::pin(stream)))
    }

    async fn client_heartbeat(
        &self,
        request: Request<ClientHeartbeatRequest>,
    ) -> Result<Response<ClientHeartbeatResponse>, Status> {
        let req = request.into_inner();
        debug!("ClientHeartbeat from client_id={}", req.client_id);

        metrics::counter!(srv_metrics::GRPC_REQUESTS_TOTAL, "method" => "client_heartbeat").increment(1);

        // Refresh connection tracking — update heartbeat if connected, otherwise register
        if self.state.connection_service.is_connected(&req.client_id) {
            self.state.connection_service.touch_heartbeat(&req.client_id);
        } else {
            self.state
                .connection_service
                .connect(req.client_id.clone(), String::new());
        }

        Ok(Response::new(ClientHeartbeatResponse {
            success: true,
            server_timestamp: chrono::Utc::now().timestamp_millis(),
        }))
    }

    async fn notify_data_change(
        &self,
        request: Request<DataChangeNotification>,
    ) -> Result<Response<DataChangeNotificationResponse>, Status> {
        let req = request.into_inner();
        debug!(
            "NotifyDataChange: data_info_id={} version={} from={}",
            req.data_info_id, req.version, req.data_server_address
        );

        metrics::counter!(srv_metrics::GRPC_REQUESTS_TOTAL, "method" => "notify_data_change").increment(1);

        // Check if the version is newer than what we have cached
        let remote_version = sofa_registry_core::model::DatumVersion { value: req.version };
        if !self.state.cache_service.is_stale(&req.data_info_id, &remote_version) {
            debug!("Data unchanged for {}, skipping push", req.data_info_id);
            return Ok(Response::new(DataChangeNotificationResponse { success: true }));
        }

        // Update cache
        self.state.cache_service.update_version(&req.data_info_id, remote_version);

        // Get subscribers for this data_info_id
        let subscribers = self.state.subscriber_registry.get_by_data_info_id(&req.data_info_id);
        if subscribers.is_empty() {
            debug!("No subscribers for {}", req.data_info_id);
            return Ok(Response::new(DataChangeNotificationResponse { success: true }));
        }

        let subscriber_ids: Vec<String> = subscribers.iter().map(|s| s.client_id.clone()).collect();

        // Create a push task with placeholder data.
        // In a full implementation, we would fetch the actual data from the data server.
        // For now, we create a minimal ReceivedData with version info.
        let received_data = sofa_registry_core::model::ReceivedData {
            data_id: req.data_info_id.clone(),
            group: String::new(),
            instance_id: String::new(),
            segment: None,
            scope: None,
            subscriber_regist_ids: subscriber_ids.clone(),
            data: std::collections::HashMap::new(),
            version: Some(req.version),
            local_zone: None,
            data_count: std::collections::HashMap::new(),
        };

        self.state
            .push_service
            .push(crate::push::PushTask {
                data_info_id: req.data_info_id,
                subscriber_regist_ids: subscriber_ids,
                data: received_data,
            })
            .await;

        Ok(Response::new(DataChangeNotificationResponse { success: true }))
    }
}
