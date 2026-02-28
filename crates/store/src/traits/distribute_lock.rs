use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sofa_registry_core::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributeLock {
    pub data_center: String,
    pub lock_name: String,
    pub owner: String,
    pub duration: i64,
    pub term: i64,
    pub term_duration: i64,
    pub gmt_create: DateTime<Utc>,
    pub gmt_modified: DateTime<Utc>,
}

impl DistributeLock {
    pub fn is_expired(&self) -> bool {
        let expire_time = self.gmt_modified.timestamp_millis() + self.duration;
        Utc::now().timestamp_millis() > expire_time
    }

    pub fn expire_timestamp(&self) -> i64 {
        self.gmt_modified.timestamp_millis() + self.duration
    }
}

#[async_trait]
pub trait DistributeLockRepository: Send + Sync {
    /// Try to acquire a lock by inserting (first time) or updating (re-election).
    async fn compete_lock(
        &self,
        lock_name: &str,
        data_center: &str,
        owner: &str,
        duration_ms: i64,
    ) -> Result<Option<DistributeLock>>;

    /// Query current lock holder.
    async fn query_lock(
        &self,
        lock_name: &str,
        data_center: &str,
    ) -> Result<Option<DistributeLock>>;

    /// Heartbeat to refresh lock ownership duration.
    async fn owner_heartbeat(
        &self,
        lock_name: &str,
        data_center: &str,
        owner: &str,
        duration_ms: i64,
    ) -> Result<bool>;
}
