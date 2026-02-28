use std::sync::Arc;

use sofa_registry_core::model::{
    ConnectId, ProcessId, PublishSource, PublishType, Publisher, RegisterVersion,
    ServerDataBox,
};
use sofa_registry_core::pb::sofa::registry::data::data_service_server::DataService;
use sofa_registry_core::pb::sofa::registry::data::{
    BatchPutDataRequest, BatchPutDataResponse, DatumDigestPb, GetDataRequest, GetDataResponse,
    NotifyDataChangeRequest, NotifyDataChangeResponse, PublishDataRequest, PublishDataResponse,
    PublisherPb, SlotDiffDigestRequest, SlotDiffDigestResponse, SlotDiffPublisherRequest,
    SlotDiffPublisherResponse, SubDatumPb, UnPublishDataRequest, UnPublishDataResponse,
};
use sofa_registry_server_shared::metrics as srv_metrics;
use sofa_registry_store::traits::DatumStorage;
use tonic::{Request, Response, Status};
use tracing::debug;

use crate::change::{DataChangeEvent, DataChangeEventCenter};
use crate::lease::SessionLeaseManager;
use crate::replication::SlotDiffSyncer;
use crate::slot::DataSlotManager;

/// gRPC service implementation for the Data server.
pub struct DataGrpcService {
    storage: Arc<dyn DatumStorage>,
    slot_manager: Arc<DataSlotManager>,
    change_center: DataChangeEventCenter,
    diff_syncer: Arc<SlotDiffSyncer>,
    data_center: String,
    session_lease_manager: Arc<SessionLeaseManager>,
}

impl DataGrpcService {
    pub fn new(
        storage: Arc<dyn DatumStorage>,
        slot_manager: Arc<DataSlotManager>,
        change_center: DataChangeEventCenter,
        diff_syncer: Arc<SlotDiffSyncer>,
        data_center: String,
        session_lease_manager: Arc<SessionLeaseManager>,
    ) -> Self {
        Self {
            storage,
            slot_manager,
            change_center,
            diff_syncer,
            data_center,
            session_lease_manager,
        }
    }

    #[allow(clippy::result_large_err)]
    fn check_slot_leader(&self, slot_id: u32) -> Result<(), Status> {
        if !self.slot_manager.am_i_leader(slot_id) {
            let leader = self
                .slot_manager
                .get_leader_for_slot(slot_id)
                .unwrap_or_default();
            return Err(sofa_registry_core::error::RegistryError::SlotMoved {
                slot_id,
                new_leader: leader,
            }
            .into());
        }
        Ok(())
    }
}

/// Convert a protobuf PublisherPb into a domain Publisher.
fn pb_to_publisher(pb: &PublisherPb) -> Publisher {
    let process_id = parse_process_id(&pb.process_id);
    let session_process_id = parse_process_id(&pb.session_process_id);
    let source_address = parse_connect_id(&pb.source_address);

    Publisher {
        data_info_id: pb.data_info_id.clone(),
        data_id: pb.data_id.clone(),
        instance_id: pb.instance_id.clone(),
        group: pb.group.clone(),
        regist_id: pb.regist_id.clone(),
        client_id: pb.client_id.clone(),
        cell: if pb.cell.is_empty() {
            None
        } else {
            Some(pb.cell.clone())
        },
        app_name: if pb.app_name.is_empty() {
            None
        } else {
            Some(pb.app_name.clone())
        },
        process_id,
        version: RegisterVersion::new(pb.version, pb.version_timestamp),
        source_address,
        session_process_id,
        data_list: pb
            .data_list
            .iter()
            .map(|b| ServerDataBox::new(bytes::Bytes::copy_from_slice(b)))
            .collect(),
        publish_type: match pb.publish_type.as_str() {
            "TEMPORARY" => PublishType::Temporary,
            _ => PublishType::Normal,
        },
        publish_source: match pb.publish_source.as_str() {
            "SESSION_SYNC" => PublishSource::SessionSync,
            _ => PublishSource::Client,
        },
        attributes: pb.attributes.clone(),
        register_timestamp: pb.register_timestamp,
    }
}

/// Convert a domain Publisher to protobuf.
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

fn parse_process_id(s: &str) -> ProcessId {
    // Format: "host-timestamp-sequence"
    let parts: Vec<&str> = s.splitn(3, '-').collect();
    if parts.len() == 3 {
        ProcessId::new(
            parts[0],
            parts[1].parse().unwrap_or(0),
            parts[2].parse().unwrap_or(0),
        )
    } else {
        ProcessId::new(s, 0, 0)
    }
}

fn parse_connect_id(s: &str) -> ConnectId {
    // Format: "clientHost:clientPort-serverHost:serverPort"
    let parts: Vec<&str> = s.splitn(2, '-').collect();
    if parts.len() == 2 {
        let client_parts: Vec<&str> = parts[0].rsplitn(2, ':').collect();
        let server_parts: Vec<&str> = parts[1].rsplitn(2, ':').collect();
        if client_parts.len() == 2 && server_parts.len() == 2 {
            return ConnectId::new(
                client_parts[1],
                client_parts[0].parse().unwrap_or(0),
                server_parts[1],
                server_parts[0].parse().unwrap_or(0),
            );
        }
    }
    ConnectId::new(s, 0, "", 0)
}

#[tonic::async_trait]
impl DataService for DataGrpcService {
    async fn get_data(
        &self,
        request: Request<GetDataRequest>,
    ) -> Result<Response<GetDataResponse>, Status> {
        metrics::counter!(srv_metrics::GRPC_REQUESTS_TOTAL, "method" => "get_data").increment(1);

        let req = request.into_inner();
        let data_center = if req.data_center.is_empty() {
            &self.data_center
        } else {
            &req.data_center
        };
        let slot_id = req.slot_id;

        debug!(
            "GetData: dc={} data_info_id={} slot={}",
            data_center, req.data_info_id, slot_id
        );

        self.check_slot_leader(slot_id)?;

        let datum = self.storage.get(data_center, &req.data_info_id);
        match datum {
            Some(d) => {
                let publishers: Vec<PublisherPb> =
                    d.pub_map.values().map(publisher_to_pb).collect();
                let sub_datum = SubDatumPb {
                    data_info_id: d.data_info_id,
                    data_center: d.data_center,
                    data_id: d.data_id,
                    instance_id: d.instance_id,
                    group: d.group,
                    publishers,
                    version: d.version.value,
                };
                Ok(Response::new(GetDataResponse {
                    success: true,
                    slot_id,
                    status: "OK".to_string(),
                    datum: Some(sub_datum),
                }))
            }
            None => Ok(Response::new(GetDataResponse {
                success: true,
                slot_id,
                status: "NOT_FOUND".to_string(),
                datum: None,
            })),
        }
    }

    async fn publish_data(
        &self,
        request: Request<PublishDataRequest>,
    ) -> Result<Response<PublishDataResponse>, Status> {
        metrics::counter!(srv_metrics::GRPC_REQUESTS_TOTAL, "method" => "publish_data").increment(1);

        let req = request.into_inner();
        let data_center = if req.data_center.is_empty() {
            &self.data_center
        } else {
            &req.data_center
        };
        let slot_id = req.slot_id;

        self.check_slot_leader(slot_id)?;

        // Renew session lease if we have session info from publishers
        for pub_pb in &req.publishers {
            if !pub_pb.session_process_id.is_empty() {
                self.session_lease_manager
                    .renew(&pub_pb.session_process_id, &pub_pb.session_process_id);
            }
        }

        for pub_pb in &req.publishers {
            let publisher = pb_to_publisher(pub_pb);
            let data_info_id = publisher.data_info_id.clone();
            let version = self.storage.put_publisher(data_center, publisher);

            self.change_center.on_change(DataChangeEvent {
                data_center: data_center.to_string(),
                data_info_id,
                version,
            });
        }

        let count = self.storage.datum_count(data_center);
        metrics::gauge!(srv_metrics::DATA_DATUM_COUNT).set(count as f64);

        Ok(Response::new(PublishDataResponse {
            success: true,
            slot_id,
            status: "OK".to_string(),
        }))
    }

    async fn un_publish_data(
        &self,
        request: Request<UnPublishDataRequest>,
    ) -> Result<Response<UnPublishDataResponse>, Status> {
        metrics::counter!(srv_metrics::GRPC_REQUESTS_TOTAL, "method" => "un_publish_data").increment(1);

        let req = request.into_inner();
        let data_center = if req.data_center.is_empty() {
            &self.data_center
        } else {
            &req.data_center
        };
        let slot_id = req.slot_id;

        self.check_slot_leader(slot_id)?;

        for regist_id in &req.regist_ids {
            if let Some(version) =
                self.storage
                    .remove_publisher(data_center, &req.data_info_id, regist_id)
            {
                self.change_center.on_change(DataChangeEvent {
                    data_center: data_center.to_string(),
                    data_info_id: req.data_info_id.clone(),
                    version,
                });
            }
        }

        Ok(Response::new(UnPublishDataResponse {
            success: true,
            slot_id,
            status: "OK".to_string(),
        }))
    }

    async fn batch_put_data(
        &self,
        request: Request<BatchPutDataRequest>,
    ) -> Result<Response<BatchPutDataResponse>, Status> {
        metrics::counter!(srv_metrics::GRPC_REQUESTS_TOTAL, "method" => "batch_put_data").increment(1);

        let req = request.into_inner();
        let data_center = if req.data_center.is_empty() {
            &self.data_center
        } else {
            &req.data_center
        };
        let slot_id = req.slot_id;

        self.check_slot_leader(slot_id)?;

        // Renew session lease
        if !req.session_process_id.is_empty() {
            self.session_lease_manager
                .renew(&req.session_process_id, &req.session_process_id);
        }

        // Put publishers
        for pub_pb in &req.publishers {
            let publisher = pb_to_publisher(pub_pb);
            let data_info_id = publisher.data_info_id.clone();
            let version = self.storage.put_publisher(data_center, publisher);

            self.change_center.on_change(DataChangeEvent {
                data_center: data_center.to_string(),
                data_info_id,
                version,
            });
        }

        // Remove unpublishers (we need the data_info_id from publisher context;
        // the proto includes session_process_id for bulk session cleanup)
        if !req.session_process_id.is_empty() && !req.unpublisher_regist_ids.is_empty() {
            // Unpublish by session is handled via the session_process_id field.
            // Individual regist_id removal would need a data_info_id, which is not
            // in the batch proto for unpublishers. This matches the Java behavior
            // where batch unpublish goes through session cleanup.
            let session_pid = parse_process_id(&req.session_process_id);
            let updated = self
                .storage
                .remove_publishers_by_session(data_center, &session_pid);
            for (data_info_id, version) in updated {
                self.change_center.on_change(DataChangeEvent {
                    data_center: data_center.to_string(),
                    data_info_id,
                    version,
                });
            }
        }

        Ok(Response::new(BatchPutDataResponse {
            success: true,
            slot_id,
            status: "OK".to_string(),
        }))
    }

    async fn slot_diff_digest(
        &self,
        request: Request<SlotDiffDigestRequest>,
    ) -> Result<Response<SlotDiffDigestResponse>, Status> {
        metrics::counter!(srv_metrics::GRPC_REQUESTS_TOTAL, "method" => "slot_diff_data").increment(1);

        let req = request.into_inner();
        let data_center = if req.data_center.is_empty() {
            &self.data_center
        } else {
            &req.data_center
        };
        let slot_id = req.slot_id;

        // Leader computes its own digest for the slot.
        let local_digests = self.diff_syncer.compute_slot_digest(data_center, slot_id);

        // Convert the follower's digests from protobuf.
        let remote_digests: Vec<crate::replication::DatumDigest> = req
            .digests
            .iter()
            .map(|d| crate::replication::DatumDigest {
                data_info_id: d.data_info_id.clone(),
                version: d.version,
                publisher_count: d.publisher_count,
            })
            .collect();

        // Diff: what the follower needs to update/remove.
        let diff = SlotDiffSyncer::diff_digests(&remote_digests, &local_digests);

        // Build updated digests for the follower.
        let updated_digests: Vec<DatumDigestPb> = local_digests
            .iter()
            .filter(|d| diff.updated.contains(&d.data_info_id))
            .map(|d| DatumDigestPb {
                data_info_id: d.data_info_id.clone(),
                version: d.version,
                publisher_count: d.publisher_count,
            })
            .collect();

        Ok(Response::new(SlotDiffDigestResponse {
            success: true,
            updated_data_info_ids: diff.updated,
            removed_data_info_ids: diff.removed,
            updated_digests,
        }))
    }

    async fn slot_diff_publisher(
        &self,
        request: Request<SlotDiffPublisherRequest>,
    ) -> Result<Response<SlotDiffPublisherResponse>, Status> {
        let req = request.into_inner();
        let data_center = if req.data_center.is_empty() {
            &self.data_center
        } else {
            &req.data_center
        };

        let mut datums = Vec::new();
        for data_info_id in &req.data_info_ids {
            if let Some(datum) = self.storage.get(data_center, data_info_id) {
                let publishers: Vec<PublisherPb> =
                    datum.pub_map.values().map(publisher_to_pb).collect();
                datums.push(SubDatumPb {
                    data_info_id: datum.data_info_id,
                    data_center: datum.data_center,
                    data_id: datum.data_id,
                    instance_id: datum.instance_id,
                    group: datum.group,
                    publishers,
                    version: datum.version.value,
                });
            }
        }

        Ok(Response::new(SlotDiffPublisherResponse {
            success: true,
            datums,
        }))
    }

    async fn notify_data_change(
        &self,
        request: Request<NotifyDataChangeRequest>,
    ) -> Result<Response<NotifyDataChangeResponse>, Status> {
        let req = request.into_inner();
        debug!(
            "NotifyDataChange: dc={} data_info_id={} version={}",
            req.data_center, req.data_info_id, req.version
        );

        // This RPC is sent FROM data server TO session servers.
        // If we receive it here, it means another data server is forwarding a change.
        // Just log and ack.
        Ok(Response::new(NotifyDataChangeResponse { success: true }))
    }
}
