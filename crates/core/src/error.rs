use thiserror::Error;

#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("Slot table error: {0}")]
    SlotTable(String),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Remoting error: {0}")]
    Remoting(String),

    #[error("Config error: {0}")]
    Config(String),

    #[error("Auth error: {0}")]
    Auth(String),

    #[error("Not leader")]
    NotLeader,

    #[error("Slot moved: slot {slot_id} is now at {new_leader}")]
    SlotMoved { slot_id: u32, new_leader: String },

    #[error("Slot access denied: {0}")]
    SlotAccessDenied(String),

    #[error("Registration refused: {0}")]
    Refused(String),

    #[error("Duplicate registration: {0}")]
    Duplicate(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Timeout: {0}")]
    Timeout(String),

    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, RegistryError>;

/// Convert `RegistryError` into a `tonic::Status` for gRPC responses.
impl From<RegistryError> for tonic::Status {
    fn from(err: RegistryError) -> Self {
        match &err {
            RegistryError::NotLeader => {
                tonic::Status::failed_precondition(err.to_string())
            }
            RegistryError::SlotMoved { .. } => {
                tonic::Status::unavailable(err.to_string())
            }
            RegistryError::SlotAccessDenied(_) => {
                tonic::Status::permission_denied(err.to_string())
            }
            RegistryError::Refused(_) => {
                tonic::Status::permission_denied(err.to_string())
            }
            RegistryError::Duplicate(_) => {
                tonic::Status::already_exists(err.to_string())
            }
            RegistryError::NotFound(_) => {
                tonic::Status::not_found(err.to_string())
            }
            RegistryError::Auth(_) => {
                tonic::Status::unauthenticated(err.to_string())
            }
            RegistryError::Timeout(_) => {
                tonic::Status::deadline_exceeded(err.to_string())
            }
            RegistryError::Config(_) => {
                tonic::Status::invalid_argument(err.to_string())
            }
            RegistryError::Connection(_) | RegistryError::Remoting(_) => {
                tonic::Status::unavailable(err.to_string())
            }
            _ => tonic::Status::internal(err.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_leader_maps_to_failed_precondition() {
        let status: tonic::Status = RegistryError::NotLeader.into();
        assert_eq!(status.code(), tonic::Code::FailedPrecondition);
    }

    #[test]
    fn not_found_maps_to_not_found() {
        let status: tonic::Status = RegistryError::NotFound("x".into()).into();
        assert_eq!(status.code(), tonic::Code::NotFound);
        assert!(status.message().contains("x"));
    }

    #[test]
    fn auth_maps_to_unauthenticated() {
        let status: tonic::Status = RegistryError::Auth("bad token".into()).into();
        assert_eq!(status.code(), tonic::Code::Unauthenticated);
    }

    #[test]
    fn timeout_maps_to_deadline_exceeded() {
        let status: tonic::Status = RegistryError::Timeout("5s".into()).into();
        assert_eq!(status.code(), tonic::Code::DeadlineExceeded);
    }

    #[test]
    fn duplicate_maps_to_already_exists() {
        let status: tonic::Status = RegistryError::Duplicate("dup".into()).into();
        assert_eq!(status.code(), tonic::Code::AlreadyExists);
    }

    #[test]
    fn slot_moved_maps_to_unavailable() {
        let status: tonic::Status = RegistryError::SlotMoved {
            slot_id: 1,
            new_leader: "host:9621".into(),
        }
        .into();
        assert_eq!(status.code(), tonic::Code::Unavailable);
    }

    #[test]
    fn connection_error_maps_to_unavailable() {
        let status: tonic::Status = RegistryError::Connection("refused".into()).into();
        assert_eq!(status.code(), tonic::Code::Unavailable);
    }

    #[test]
    fn internal_error_maps_to_internal() {
        let status: tonic::Status = RegistryError::Internal("oops".into()).into();
        assert_eq!(status.code(), tonic::Code::Internal);
    }

    #[test]
    fn config_maps_to_invalid_argument() {
        let status: tonic::Status = RegistryError::Config("bad port".into()).into();
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
    }

    #[test]
    fn storage_maps_to_internal() {
        let status: tonic::Status = RegistryError::Storage("corrupt".into()).into();
        assert_eq!(status.code(), tonic::Code::Internal);
    }
}
