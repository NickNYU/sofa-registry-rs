use std::sync::Arc;
use tonic::{Request, Response, Status};
use tracing::{info, debug};
use sofa_registry_store::traits::leader_elector::LeaderElector;
use sofa_registry_core::pb::sofa::registry::meta::{
    meta_service_server::MetaService,
    HeartbeatRequest, HeartbeatResponse,
    GetSlotTableRequest, GetSlotTableResponse,
    RegisterNodeRequest, RegisterNodeResponse,
    RenewNodeRequest, RenewNodeResponse,
    GetLeaderRequest, GetLeaderResponse,
    SlotTablePb, SlotPb,
};
use sofa_registry_core::slot::SlotTable;
use sofa_registry_server_shared::metrics as srv_metrics;
use crate::server::MetaServerState;
use crate::lease::data_server_manager::DataNode;
use crate::lease::session_server_manager::SessionNode;

pub struct MetaGrpcServiceImpl {
    state: Arc<MetaServerState>,
}

impl MetaGrpcServiceImpl {
    pub fn new(state: Arc<MetaServerState>) -> Self {
        Self { state }
    }
}

fn slot_table_to_pb(table: &SlotTable) -> SlotTablePb {
    SlotTablePb {
        epoch: table.epoch,
        slots: table.slots.values().map(|s| SlotPb {
            id: s.id,
            leader: s.leader.clone(),
            leader_epoch: s.leader_epoch,
            followers: s.followers.iter().cloned().collect(),
        }).collect(),
    }
}

#[tonic::async_trait]
impl MetaService for MetaGrpcServiceImpl {
    async fn heartbeat(
        &self,
        request: Request<HeartbeatRequest>,
    ) -> Result<Response<HeartbeatResponse>, Status> {
        let req = request.into_inner();
        debug!("Heartbeat from {} ({})", req.address, req.node_type);

        let success = match req.node_type.as_str() {
            "DATA" => {
                metrics::counter!(srv_metrics::GRPC_REQUESTS_TOTAL, "method" => "heartbeat_data_server").increment(1);
                let result = self.state.data_server_manager.renew(&req.address);
                metrics::gauge!(srv_metrics::META_DATA_SERVERS).set(self.state.data_server_manager.count() as f64);
                result
            }
            "SESSION" => {
                metrics::counter!(srv_metrics::GRPC_REQUESTS_TOTAL, "method" => "heartbeat_session_server").increment(1);
                let result = self.state.session_server_manager.renew(&req.address);
                metrics::gauge!(srv_metrics::META_SESSION_SERVERS).set(self.state.session_server_manager.count() as f64);
                result
            }
            _ => false,
        };

        Ok(Response::new(HeartbeatResponse {
            success,
            slot_table_epoch: self.state.slot_manager.get_epoch(),
        }))
    }

    async fn get_slot_table(
        &self,
        request: Request<GetSlotTableRequest>,
    ) -> Result<Response<GetSlotTableResponse>, Status> {
        metrics::counter!(srv_metrics::GRPC_REQUESTS_TOTAL, "method" => "get_slot_table").increment(1);
        let req = request.into_inner();
        let table = self.state.slot_manager.get_slot_table();
        
        // If client's epoch matches, return unchanged
        if req.current_epoch == table.epoch {
            return Ok(Response::new(GetSlotTableResponse {
                success: true,
                slot_table: None,
                unchanged: true,
            }));
        }

        Ok(Response::new(GetSlotTableResponse {
            success: true,
            slot_table: Some(slot_table_to_pb(&table)),
            unchanged: false,
        }))
    }

    async fn register_node(
        &self,
        request: Request<RegisterNodeRequest>,
    ) -> Result<Response<RegisterNodeResponse>, Status> {
        let req = request.into_inner();
        info!("RegisterNode: type={}, address={}", req.node_type, req.address);

        match req.node_type.as_str() {
            "DATA" => {
                let node = DataNode::new(&req.address, &req.data_center, &req.cluster_id);
                self.state.data_server_manager.register(node);
                
                // Try to assign/rebalance slots when a new data server joins
                if self.state.leader_elector.am_i_leader() {
                    self.state.slot_manager.try_assign_or_rebalance();
                }
            }
            "SESSION" => {
                let node = SessionNode::new(&req.address, &req.data_center, &req.cluster_id);
                self.state.session_server_manager.register(node);
            }
            other => {
                return Err(Status::invalid_argument(format!("Unknown node type: {}", other)));
            }
        }

        let table = self.state.slot_manager.get_slot_table();
        Ok(Response::new(RegisterNodeResponse {
            success: true,
            message: "Registered".to_string(),
            slot_table: Some(slot_table_to_pb(&table)),
        }))
    }

    async fn renew_node(
        &self,
        request: Request<RenewNodeRequest>,
    ) -> Result<Response<RenewNodeResponse>, Status> {
        metrics::counter!(srv_metrics::GRPC_REQUESTS_TOTAL, "method" => "renew_node").increment(1);
        let req = request.into_inner();
        
        let success = match req.node_type.as_str() {
            "DATA" => self.state.data_server_manager.renew(&req.address),
            "SESSION" => self.state.session_server_manager.renew(&req.address),
            _ => false,
        };

        Ok(Response::new(RenewNodeResponse {
            success,
            slot_table_epoch: self.state.slot_manager.get_epoch(),
        }))
    }

    async fn get_leader(
        &self,
        _request: Request<GetLeaderRequest>,
    ) -> Result<Response<GetLeaderResponse>, Status> {
        let info = self.state.leader_elector.get_leader_info();
        Ok(Response::new(GetLeaderResponse {
            leader: info.leader.unwrap_or_default(),
            epoch: info.epoch,
            expire_timestamp: info.expire_timestamp,
        }))
    }
}
