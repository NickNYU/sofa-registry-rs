use std::sync::Arc;

use sofa_registry_core::model::{PublishSource, PublishType, Publisher};
use sofa_registry_core::pb::sofa::registry::data::data_service_client::DataServiceClient;
use sofa_registry_core::pb::sofa::registry::data::{
    PublishDataRequest, PublisherPb, UnPublishDataRequest,
};
use sofa_registry_remoting::GrpcClientPool;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use sofa_registry_server_shared::metrics as srv_metrics;

use crate::slot::SessionSlotManager;

/// Describes a write that should be forwarded to the data server.
pub enum WriteRequest {
    Publish { publisher: Box<Publisher> },
    Unpublish { data_info_id: String, regist_id: String },
}

/// Sender side: accepts writes and enqueues them for batch forwarding.
pub struct WriteDataAcceptor {
    tx: mpsc::Sender<WriteRequest>,
}

impl WriteDataAcceptor {
    /// Create a new write acceptor with the given buffer size.
    /// Returns the acceptor (sender) and the receiver that processes requests.
    pub fn new(
        buffer_size: usize,
        pool: Arc<GrpcClientPool>,
        slot_manager: Arc<SessionSlotManager>,
        data_center: String,
        session_process_id: String,
    ) -> (Self, WriteDataReceiver) {
        let (tx, rx) = mpsc::channel(buffer_size);
        (
            Self { tx },
            WriteDataReceiver {
                rx,
                pool,
                slot_manager,
                data_center,
                session_process_id,
            },
        )
    }

    /// Enqueue a write request. Logs a warning if the channel is full.
    pub async fn accept(&self, request: WriteRequest) {
        if let Err(e) = self.tx.try_send(request) {
            match e {
                mpsc::error::TrySendError::Full(_) => {
                    warn!("Write data channel full, dropping write request");
                }
                mpsc::error::TrySendError::Closed(_) => {
                    warn!("Write data channel closed");
                }
            }
        }
    }
}

/// Receiver side: processes enqueued writes by forwarding them to the
/// appropriate data server via gRPC, using slot-based routing.
pub struct WriteDataReceiver {
    rx: mpsc::Receiver<WriteRequest>,
    pool: Arc<GrpcClientPool>,
    slot_manager: Arc<SessionSlotManager>,
    data_center: String,
    session_process_id: String,
}

impl WriteDataReceiver {
    /// Process write requests until the cancellation token fires.
    pub async fn run(mut self, cancel: CancellationToken) {
        info!("WriteDataReceiver started");
        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    info!("WriteDataReceiver shutting down");
                    break;
                }
                req = self.rx.recv() => {
                    match req {
                        Some(write_req) => {
                            self.process_write(write_req).await;
                        }
                        None => {
                            info!("Write data channel closed, exiting");
                            break;
                        }
                    }
                }
            }
        }
    }

    async fn process_write(&self, write_req: WriteRequest) {
        match write_req {
            WriteRequest::Publish { publisher } => {
                self.forward_publish(*publisher).await;
            }
            WriteRequest::Unpublish {
                data_info_id,
                regist_id,
            } => {
                self.forward_unpublish(&data_info_id, &regist_id).await;
            }
        }
    }

    async fn forward_publish(&self, publisher: Publisher) {
        let data_info_id = &publisher.data_info_id;

        metrics::counter!(srv_metrics::SESSION_WRITE_FORWARDS_TOTAL, "op" => "publish").increment(1);

        let (slot_id, leader) = match self.slot_manager.get_leader_for_data(data_info_id) {
            Some(v) => v,
            None => {
                warn!(
                    "No slot leader for data_info_id={}, dropping publish",
                    data_info_id
                );
                metrics::counter!(srv_metrics::SESSION_WRITE_FORWARDS_FAILED, "op" => "publish").increment(1);
                return;
            }
        };

        debug!(
            "Forwarding publish: data_info_id={} slot={} leader={}",
            data_info_id, slot_id, leader
        );

        let channel = match self.pool.get_channel(&leader).await {
            Ok(ch) => ch,
            Err(e) => {
                error!("Failed to connect to data server {}: {}", leader, e);
                self.pool.remove_channel(&leader);
                metrics::counter!(srv_metrics::SESSION_WRITE_FORWARDS_FAILED, "op" => "publish").increment(1);
                return;
            }
        };

        let mut client = DataServiceClient::new(channel);
        let request = PublishDataRequest {
            data_center: self.data_center.clone(),
            slot_id,
            slot_table_epoch: self.slot_manager.get_epoch(),
            slot_leader_epoch: 0,
            publishers: vec![publisher_to_pb(&publisher)],
        };

        match client.publish_data(request).await {
            Ok(resp) => {
                let inner = resp.into_inner();
                if !inner.success {
                    warn!(
                        "Data server rejected publish for slot {}: {}",
                        slot_id, inner.status
                    );
                    metrics::counter!(srv_metrics::SESSION_WRITE_FORWARDS_FAILED, "op" => "publish").increment(1);
                }
            }
            Err(e) => {
                error!(
                    "Failed to forward publish to data server {}: {}",
                    leader, e
                );
                self.pool.remove_channel(&leader);
                metrics::counter!(srv_metrics::SESSION_WRITE_FORWARDS_FAILED, "op" => "publish").increment(1);
            }
        }
    }

    async fn forward_unpublish(&self, data_info_id: &str, regist_id: &str) {
        metrics::counter!(srv_metrics::SESSION_WRITE_FORWARDS_TOTAL, "op" => "unpublish").increment(1);

        let (slot_id, leader) = match self.slot_manager.get_leader_for_data(data_info_id) {
            Some(v) => v,
            None => {
                warn!(
                    "No slot leader for data_info_id={}, dropping unpublish",
                    data_info_id
                );
                metrics::counter!(srv_metrics::SESSION_WRITE_FORWARDS_FAILED, "op" => "unpublish").increment(1);
                return;
            }
        };

        debug!(
            "Forwarding unpublish: data_info_id={} regist_id={} slot={} leader={}",
            data_info_id, regist_id, slot_id, leader
        );

        let channel = match self.pool.get_channel(&leader).await {
            Ok(ch) => ch,
            Err(e) => {
                error!("Failed to connect to data server {}: {}", leader, e);
                self.pool.remove_channel(&leader);
                metrics::counter!(srv_metrics::SESSION_WRITE_FORWARDS_FAILED, "op" => "unpublish").increment(1);
                return;
            }
        };

        let mut client = DataServiceClient::new(channel);
        let request = UnPublishDataRequest {
            data_center: self.data_center.clone(),
            slot_id,
            slot_table_epoch: self.slot_manager.get_epoch(),
            slot_leader_epoch: 0,
            regist_ids: vec![regist_id.to_string()],
            data_info_id: data_info_id.to_string(),
            session_process_id: self.session_process_id.clone(),
        };

        match client.un_publish_data(request).await {
            Ok(resp) => {
                let inner = resp.into_inner();
                if !inner.success {
                    warn!(
                        "Data server rejected unpublish for slot {}: {}",
                        slot_id, inner.status
                    );
                    metrics::counter!(srv_metrics::SESSION_WRITE_FORWARDS_FAILED, "op" => "unpublish").increment(1);
                }
            }
            Err(e) => {
                error!(
                    "Failed to forward unpublish to data server {}: {}",
                    leader, e
                );
                self.pool.remove_channel(&leader);
                metrics::counter!(srv_metrics::SESSION_WRITE_FORWARDS_FAILED, "op" => "unpublish").increment(1);
            }
        }
    }
}

/// Convert a domain Publisher to protobuf PublisherPb for gRPC transmission.
fn publisher_to_pb(p: &Publisher) -> PublisherPb {
    PublisherPb {
        data_info_id: p.data_info_id.clone(),
        data_id: p.data_id.clone(),
        instance_id: p.instance_id.clone(),
        group: p.group.clone(),
        regist_id: p.regist_id.clone(),
        client_id: p.client_id.clone(),
        cell: p.cell.clone().unwrap_or_default(),
        app_name: p.app_name.clone().unwrap_or_default(),
        process_id: p.process_id.to_string(),
        version: p.version.version,
        version_timestamp: p.version.timestamp,
        source_address: p.source_address.to_string(),
        session_process_id: p.session_process_id.to_string(),
        data_list: p.data_list.iter().map(|d| d.data.to_vec()).collect(),
        publish_type: match p.publish_type {
            PublishType::Temporary => "TEMPORARY".to_string(),
            PublishType::Normal => "NORMAL".to_string(),
        },
        publish_source: match p.publish_source {
            PublishSource::SessionSync => "SESSION_SYNC".to_string(),
            PublishSource::Client => "CLIENT".to_string(),
        },
        attributes: p.attributes.clone(),
        register_timestamp: p.register_timestamp,
    }
}
