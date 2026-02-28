use async_trait::async_trait;
use sofa_registry_core::slot::SlotTable;

/// Abstraction for communication with the Meta server cluster.
///
/// Both `DataServer` and `SessionServer` use this trait to register nodes,
/// renew leases, and fetch slot tables. The concrete implementation uses gRPC
/// with automatic failover across configured meta addresses.
///
/// Providing a trait allows mock-based unit testing without spinning up
/// an actual Meta server.
#[async_trait]
pub trait MetaServiceClient: Send + Sync {
    /// Register this node with the meta cluster.
    /// Returns the initial slot table if one is available.
    async fn register_node(&self) -> Result<Option<SlotTable>, MetaError>;

    /// Renew this node's lease for the given duration.
    /// Returns the current slot table epoch from the meta server.
    async fn renew_node(&self, duration_secs: u64) -> Result<i64, MetaError>;

    /// Fetch the slot table from the meta cluster.
    /// Returns `Ok(None)` if the table is unchanged (same epoch as `current_epoch`).
    async fn get_slot_table(&self, current_epoch: i64)
        -> Result<Option<SlotTable>, MetaError>;

    /// The server type this client represents (e.g., "SESSION", "DATA").
    fn server_type(&self) -> &'static str;
}

/// Error type for MetaServiceClient operations.
#[derive(Debug, thiserror::Error)]
pub enum MetaError {
    #[error("Connection failed: {0}")]
    Connection(String),

    #[error("RPC failed: {0}")]
    Rpc(String),

    #[error("Request rejected: {0}")]
    Rejected(String),

    #[error("All meta addresses failed for {0}")]
    AllAddressesFailed(String),
}

impl From<MetaError> for sofa_registry_core::error::RegistryError {
    fn from(err: MetaError) -> Self {
        match err {
            MetaError::Connection(msg) => {
                sofa_registry_core::error::RegistryError::Connection(msg)
            }
            MetaError::Rpc(msg) => {
                sofa_registry_core::error::RegistryError::Remoting(msg)
            }
            MetaError::Rejected(msg) => {
                sofa_registry_core::error::RegistryError::Refused(msg)
            }
            MetaError::AllAddressesFailed(msg) => {
                sofa_registry_core::error::RegistryError::Connection(
                    format!("all meta addresses failed for {}", msg),
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sofa_registry_core::error::RegistryError;

    #[test]
    fn meta_error_connection_converts_to_registry_connection() {
        let err: RegistryError = MetaError::Connection("refused".into()).into();
        assert!(matches!(err, RegistryError::Connection(_)));
        assert!(err.to_string().contains("refused"));
    }

    #[test]
    fn meta_error_rpc_converts_to_registry_remoting() {
        let err: RegistryError = MetaError::Rpc("timeout".into()).into();
        assert!(matches!(err, RegistryError::Remoting(_)));
    }

    #[test]
    fn meta_error_rejected_converts_to_registry_refused() {
        let err: RegistryError = MetaError::Rejected("not leader".into()).into();
        assert!(matches!(err, RegistryError::Refused(_)));
    }

    #[test]
    fn meta_error_all_failed_converts_to_registry_connection() {
        let err: RegistryError =
            MetaError::AllAddressesFailed("register_node".into()).into();
        assert!(matches!(err, RegistryError::Connection(_)));
        assert!(err.to_string().contains("register_node"));
    }
}
