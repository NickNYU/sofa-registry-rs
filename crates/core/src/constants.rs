/// Default values matching Java SOFARegistry
pub mod defaults {
    pub const DEFAULT_GROUP: &str = "DEFAULT_GROUP";
    pub const DEFAULT_INSTANCE_ID: &str = "DEFAULT_INSTANCE_ID";
    pub const DEFAULT_DATA_CENTER: &str = "DefaultDataCenter";
    pub const DEFAULT_CLUSTER_ID: &str = "DefaultCluster";

    // Slot defaults
    pub const SLOT_NUM: u32 = 256;
    pub const SLOT_REPLICAS: u32 = 2;

    // Lease defaults
    pub const SESSION_LEASE_SECS: u64 = 30;
    pub const DATA_LEASE_SECS: u64 = 30;

    // Election defaults
    pub const ELECTION_LOCK_DURATION_MS: i64 = 30000;
    pub const META_LEADER_LOCK_NAME: &str = "META-MASTER";

    // Network defaults
    pub const META_GRPC_PORT: u16 = 9611;
    pub const META_HTTP_PORT: u16 = 9612;
    pub const DATA_GRPC_PORT: u16 = 9621;
    pub const DATA_HTTP_PORT: u16 = 9622;
    pub const DATA_SYNC_PORT: u16 = 9623;
    pub const SESSION_GRPC_PORT: u16 = 9601;
    pub const SESSION_HTTP_PORT: u16 = 9602;

    // Timing
    pub const SLOT_SYNC_INTERVAL_SECS: u64 = 6;
    pub const DATA_CHANGE_DEBOUNCE_MS: u64 = 500;
    pub const PUSH_TASK_TIMEOUT_MS: u64 = 3000;
}

/// Value type names used in registration
pub mod value_constants {
    pub const PUBLISH: &str = "PUB";
    pub const SUBSCRIBE: &str = "SUB";
    pub const UNREGISTER: &str = "UNREG";
}

/// Server role identifiers
pub mod server_type {
    pub const SESSION: &str = "SESSION";
    pub const DATA: &str = "DATA";
    pub const META: &str = "META";
}

/// Event types for registration protocol
pub mod event_type {
    pub const REGISTER: &str = "REGISTER";
    pub const UNREGISTER: &str = "UNREGISTER";
}

/// Publish type identifiers
pub mod publish_type {
    pub const NORMAL: &str = "NORMAL";
    pub const TEMPORARY: &str = "TEMPORARY";
}

/// Publish source identifiers
pub mod publish_source {
    pub const CLIENT: &str = "CLIENT";
    pub const SESSION_SYNC: &str = "SESSION_SYNC";
}

/// Subscription scope identifiers
pub mod scope {
    pub const ZONE: &str = "zone";
    pub const GLOBAL: &str = "global";
    pub const DATA_CENTER: &str = "dataCenter";
}
