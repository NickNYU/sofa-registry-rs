use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sofa_registry_core::error::Result;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderInfo {
    pub epoch: i64,
    pub leader: Option<String>,
    pub expire_timestamp: i64,
}

impl LeaderInfo {
    pub fn empty() -> Self {
        Self {
            epoch: -1,
            leader: None,
            expire_timestamp: 0,
        }
    }

    pub fn is_valid(&self) -> bool {
        self.leader.is_some() && self.expire_timestamp > Utc::now().timestamp_millis()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElectorRole {
    Leader,
    Follower,
    Observer,
}

pub trait LeaderAware: Send + Sync {
    fn on_become_leader(&self);
    fn on_lose_leadership(&self);
}

#[async_trait]
pub trait LeaderElector: Send + Sync {
    async fn elect(&self) -> Result<LeaderInfo>;
    async fn query_leader(&self) -> Result<LeaderInfo>;
    fn am_i_leader(&self) -> bool;
    fn get_leader_info(&self) -> LeaderInfo;
    fn get_role(&self) -> ElectorRole;
    fn myself(&self) -> &str;
    fn change_to_follower(&self);
    fn change_to_observer(&self);
    fn register_leader_aware(&self, aware: Arc<dyn LeaderAware>);
}
