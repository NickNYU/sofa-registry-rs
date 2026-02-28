use parking_lot::RwLock;
use sofa_registry_core::error::{RegistryError, Result};
use sofa_registry_core::model::{DataBox, ReceivedData};
use sofa_registry_core::pb::sofa::registry::session::session_service_client::SessionServiceClient;
use sofa_registry_core::pb::sofa::registry::session::{
    ClientHeartbeatRequest, SubscribeStreamRequest, UnregisterRequest,
};
use sofa_registry_core::pb::sofa::registry::{
    BaseRegisterPb, DataBoxPb, PublisherRegisterPb, RegisterResponsePb, SubscriberRegisterPb,
};
use std::collections::HashMap;
use tokio::sync::mpsc;
use tonic::transport::{Channel, Endpoint};
use tracing::{debug, info};

/// Manages the gRPC connection to a single session server.
pub struct ClientConnection {
    address: String,
    channel: RwLock<Option<Channel>>,
    connect_timeout_ms: u64,
    request_timeout_ms: u64,
}

impl ClientConnection {
    pub fn new(address: &str, connect_timeout_ms: u64, request_timeout_ms: u64) -> Self {
        Self {
            address: address.to_string(),
            channel: RwLock::new(None),
            connect_timeout_ms,
            request_timeout_ms,
        }
    }

    pub fn address(&self) -> &str {
        &self.address
    }

    pub fn is_connected(&self) -> bool {
        self.channel.read().is_some()
    }

    /// Establish the gRPC channel to the session server.
    pub async fn connect(&self) -> Result<()> {
        let endpoint = Endpoint::from_shared(format!("http://{}", self.address))
            .map_err(|e| RegistryError::Connection(format!("invalid endpoint: {}", e)))?
            .connect_timeout(std::time::Duration::from_millis(self.connect_timeout_ms))
            .timeout(std::time::Duration::from_millis(self.request_timeout_ms));

        let channel = endpoint
            .connect()
            .await
            .map_err(|e| RegistryError::Connection(format!("connect failed: {}", e)))?;

        info!("Connected to session server at {}", self.address);
        *self.channel.write() = Some(channel);
        Ok(())
    }

    /// Disconnect and drop the channel.
    pub fn disconnect(&self) {
        *self.channel.write() = None;
    }

    fn get_client(&self) -> Result<SessionServiceClient<Channel>> {
        let ch = self
            .channel
            .read()
            .clone()
            .ok_or_else(|| RegistryError::Connection("not connected".to_string()))?;
        Ok(SessionServiceClient::new(ch))
    }

    /// Register a publisher via gRPC.
    pub async fn register_publisher(
        &self,
        base: BaseRegisterPb,
        data_list: Vec<DataBoxPb>,
    ) -> Result<RegisterResponsePb> {
        let mut client = self.get_client()?;
        let req = PublisherRegisterPb {
            base: Some(base),
            data_list,
        };
        let resp = client
            .register_publisher(tonic::Request::new(req))
            .await
            .map_err(|e| RegistryError::Remoting(format!("register_publisher: {}", e)))?;
        Ok(resp.into_inner())
    }

    /// Register a subscriber via gRPC.
    pub async fn register_subscriber(
        &self,
        base: BaseRegisterPb,
        scope: String,
    ) -> Result<RegisterResponsePb> {
        let mut client = self.get_client()?;
        let req = SubscriberRegisterPb {
            base: Some(base),
            scope,
            accept_encoding: String::new(),
            accept_multi: false,
        };
        let resp = client
            .register_subscriber(tonic::Request::new(req))
            .await
            .map_err(|e| RegistryError::Remoting(format!("register_subscriber: {}", e)))?;
        Ok(resp.into_inner())
    }

    /// Unregister a publisher or subscriber.
    pub async fn unregister(
        &self,
        base: BaseRegisterPb,
        registry_type: &str,
    ) -> Result<RegisterResponsePb> {
        let mut client = self.get_client()?;
        let req = UnregisterRequest {
            base: Some(base),
            registry_type: registry_type.to_string(),
        };
        let resp = client
            .unregister(tonic::Request::new(req))
            .await
            .map_err(|e| RegistryError::Remoting(format!("unregister: {}", e)))?;
        Ok(resp.into_inner())
    }

    /// Open a subscribe stream for push notifications.
    /// Returns a channel receiver that yields ReceivedData as they arrive.
    pub async fn subscribe_stream(
        &self,
        client_id: &str,
        zone: &str,
        data_center: &str,
    ) -> Result<mpsc::UnboundedReceiver<ReceivedData>> {
        let mut client = self.get_client()?;
        let req = SubscribeStreamRequest {
            client_id: client_id.to_string(),
            zone: zone.to_string(),
            data_center: data_center.to_string(),
        };
        let resp = client
            .subscribe(tonic::Request::new(req))
            .await
            .map_err(|e| RegistryError::Remoting(format!("subscribe stream: {}", e)))?;

        let mut stream = resp.into_inner();
        let (tx, rx) = mpsc::unbounded_channel();

        tokio::spawn(async move {
            while let Ok(Some(pb)) = stream.message().await {
                let data = convert_received_data_pb(pb);
                if tx.send(data).is_err() {
                    debug!("Subscribe stream receiver dropped");
                    break;
                }
            }
            debug!("Subscribe stream ended");
        });

        Ok(rx)
    }

    /// Send a heartbeat to the session server.
    pub async fn heartbeat(&self, client_id: &str, timestamp: i64) -> Result<bool> {
        let mut client = self.get_client()?;
        let req = ClientHeartbeatRequest {
            client_id: client_id.to_string(),
            timestamp,
        };
        let resp = client
            .client_heartbeat(tonic::Request::new(req))
            .await
            .map_err(|e| RegistryError::Remoting(format!("heartbeat: {}", e)))?;
        Ok(resp.into_inner().success)
    }
}

/// Convert protobuf ReceivedDataPb to domain ReceivedData.
fn convert_received_data_pb(
    pb: sofa_registry_core::pb::sofa::registry::ReceivedDataPb,
) -> ReceivedData {
    let data: HashMap<String, Vec<DataBox>> = pb
        .data
        .into_iter()
        .map(|(k, v)| {
            let boxes = v
                .data_box
                .into_iter()
                .map(|db| {
                    if db.data.is_empty() {
                        DataBox::empty()
                    } else {
                        DataBox::new(db.data)
                    }
                })
                .collect();
            (k, boxes)
        })
        .collect();

    let data_count: HashMap<String, u32> = pb.data_count;

    ReceivedData {
        data_id: pb.data_id,
        group: pb.group,
        instance_id: pb.instance_id,
        segment: if pb.segment.is_empty() {
            None
        } else {
            Some(pb.segment)
        },
        scope: if pb.scope.is_empty() {
            None
        } else {
            Some(pb.scope)
        },
        subscriber_regist_ids: pb.subscriber_regist_ids,
        data,
        version: if pb.version == 0 {
            None
        } else {
            Some(pb.version)
        },
        local_zone: if pb.local_zone.is_empty() {
            None
        } else {
            Some(pb.local_zone)
        },
        data_count,
    }
}
