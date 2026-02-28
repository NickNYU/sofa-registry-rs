use async_trait::async_trait;
use sofa_registry_core::constants::server_type;
use sofa_registry_core::pb::sofa::registry::meta::meta_service_client::MetaServiceClient;
use sofa_registry_core::pb::sofa::registry::meta::{
    GetSlotTableRequest, RegisterNodeRequest, RenewNodeRequest,
};
use sofa_registry_core::slot::{Slot, SlotTable};
use sofa_registry_remoting::GrpcClientPool;
use sofa_registry_store::traits::MetaError;
use sofa_registry_store::traits::MetaServiceClient as MetaServiceClientTrait;
use std::collections::HashSet;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

/// Unified gRPC client for communicating with the Meta server.
///
/// Implements [`sofa_registry_store::traits::MetaServiceClient`] so it can
/// be used through the trait for testability, or directly for convenience
/// methods like `fetch_domain_slot_table` and `run_heartbeat_loop`.
pub struct MetaClient {
    pool: Arc<GrpcClientPool>,
    meta_addresses: Vec<String>,
    local_address: String,
    data_center: String,
    cluster_id: String,
    node_type: &'static str,
}

impl MetaClient {
    /// Create a new MetaClient for a Data server.
    pub fn for_data(
        meta_addresses: Vec<String>,
        local_address: String,
        data_center: String,
        cluster_id: String,
    ) -> Self {
        Self {
            pool: Arc::new(GrpcClientPool::new()),
            meta_addresses,
            local_address,
            data_center,
            cluster_id,
            node_type: server_type::DATA,
        }
    }

    /// Create a new MetaClient for a Session server.
    pub fn for_session(
        meta_addresses: Vec<String>,
        local_address: String,
        data_center: String,
        cluster_id: String,
    ) -> Self {
        Self {
            pool: Arc::new(GrpcClientPool::new()),
            meta_addresses,
            local_address,
            data_center,
            cluster_id,
            node_type: server_type::SESSION,
        }
    }

    async fn try_register(&self, addr: &str) -> Result<Option<SlotTable>, MetaError> {
        let channel = self
            .pool
            .get_channel(addr)
            .await
            .map_err(|e| MetaError::Connection(format!("{}: {}", addr, e)))?;
        let mut client = MetaServiceClient::new(channel);

        let resp = client
            .register_node(RegisterNodeRequest {
                node_type: self.node_type.to_string(),
                address: self.local_address.clone(),
                data_center: self.data_center.clone(),
                cluster_id: self.cluster_id.clone(),
                attributes: Default::default(),
            })
            .await
            .map_err(|e| MetaError::Rpc(e.to_string()))?;

        let inner = resp.into_inner();
        if inner.success {
            let slot_table = inner.slot_table.as_ref().map(pb_to_slot_table);
            info!(
                "Registered {} with meta at {}, slot_table_epoch={}",
                self.node_type,
                addr,
                slot_table.as_ref().map(|t| t.epoch).unwrap_or(0)
            );
            Ok(slot_table)
        } else {
            Err(MetaError::Rejected(inner.message))
        }
    }

    async fn try_renew(&self, addr: &str, duration_secs: u64) -> Result<i64, MetaError> {
        let channel = self
            .pool
            .get_channel(addr)
            .await
            .map_err(|e| MetaError::Connection(format!("{}: {}", addr, e)))?;
        let mut client = MetaServiceClient::new(channel);

        let resp = client
            .renew_node(RenewNodeRequest {
                node_type: self.node_type.to_string(),
                address: self.local_address.clone(),
                data_center: self.data_center.clone(),
                duration_secs: duration_secs as i64,
            })
            .await
            .map_err(|e| MetaError::Rpc(e.to_string()))?;

        let inner = resp.into_inner();
        if inner.success {
            Ok(inner.slot_table_epoch)
        } else {
            Err(MetaError::Rejected("renew rejected".to_string()))
        }
    }

    async fn try_get_slot_table(
        &self,
        addr: &str,
        current_epoch: i64,
    ) -> Result<Option<SlotTable>, MetaError> {
        let channel = self
            .pool
            .get_channel(addr)
            .await
            .map_err(|e| MetaError::Connection(format!("{}: {}", addr, e)))?;
        let mut client = MetaServiceClient::new(channel);

        let resp = client
            .get_slot_table(GetSlotTableRequest {
                data_center: self.data_center.clone(),
                current_epoch,
            })
            .await
            .map_err(|e| MetaError::Rpc(e.to_string()))?;

        let inner = resp.into_inner();
        if !inner.success {
            return Err(MetaError::Rejected("get_slot_table failed".to_string()));
        }
        if inner.unchanged {
            debug!("Slot table unchanged at epoch={}", current_epoch);
            return Ok(None);
        }
        Ok(inner.slot_table.as_ref().map(pb_to_slot_table))
    }

    /// Convenience: fetch slot table, returning None on any error.
    pub async fn fetch_domain_slot_table(&self, current_epoch: i64) -> Option<SlotTable> {
        match self.get_slot_table(current_epoch).await {
            Ok(result) => result,
            Err(e) => {
                error!("Failed to fetch slot table: {}", e);
                None
            }
        }
    }

    /// Run a periodic heartbeat loop that renews the lease at the given interval.
    pub async fn run_heartbeat_loop(
        &self,
        interval_secs: u64,
        duration_secs: u64,
        cancel: CancellationToken,
    ) {
        let mut ticker = tokio::time::interval(tokio::time::Duration::from_secs(interval_secs));
        loop {
            tokio::select! {
                biased;
                _ = cancel.cancelled() => {
                    info!("{} meta heartbeat loop shutting down", self.node_type);
                    break;
                }
                _ = ticker.tick() => {
                    if let Err(e) = self.renew_node(duration_secs).await {
                        error!("Failed to renew {} lease with meta: {}", self.node_type, e);
                    }
                }
            }
        }
    }
}

#[async_trait]
impl sofa_registry_store::traits::MetaServiceClient for MetaClient {
    async fn register_node(&self) -> Result<Option<SlotTable>, MetaError> {
        for addr in &self.meta_addresses {
            match self.try_register(addr).await {
                Ok(slot_table) => return Ok(slot_table),
                Err(e) => {
                    warn!("Failed to register with meta at {}: {}", addr, e);
                }
            }
        }
        Err(MetaError::AllAddressesFailed("register_node".to_string()))
    }

    async fn renew_node(&self, duration_secs: u64) -> Result<i64, MetaError> {
        for addr in &self.meta_addresses {
            match self.try_renew(addr, duration_secs).await {
                Ok(epoch) => return Ok(epoch),
                Err(e) => {
                    warn!("Failed to renew lease with meta at {}: {}", addr, e);
                }
            }
        }
        Err(MetaError::AllAddressesFailed("renew_node".to_string()))
    }

    async fn get_slot_table(&self, current_epoch: i64) -> Result<Option<SlotTable>, MetaError> {
        for addr in &self.meta_addresses {
            match self.try_get_slot_table(addr, current_epoch).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    warn!("Failed to fetch slot table from meta at {}: {}", addr, e);
                }
            }
        }
        Err(MetaError::AllAddressesFailed("get_slot_table".to_string()))
    }

    fn server_type(&self) -> &'static str {
        self.node_type
    }
}

/// Convert a protobuf `SlotTablePb` to the domain `SlotTable`.
pub fn pb_to_slot_table(
    pb: &sofa_registry_core::pb::sofa::registry::meta::SlotTablePb,
) -> SlotTable {
    let slots: Vec<Slot> = pb
        .slots
        .iter()
        .map(|s| {
            let followers: HashSet<String> = s.followers.iter().cloned().collect();
            Slot::new(s.id, s.leader.clone(), s.leader_epoch).with_followers(followers)
        })
        .collect();
    SlotTable::new(pb.epoch, slots)
}
