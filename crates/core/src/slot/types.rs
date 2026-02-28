use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Represents a single data partition slot
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Slot {
    /// Unique slot identifier
    pub id: u32,
    /// Address of the data server that is leader for this slot
    pub leader: String,
    /// Epoch of the leader assignment
    pub leader_epoch: i64,
    /// Set of data server addresses that are followers for this slot
    pub followers: HashSet<String>,
}

impl Slot {
    pub fn new(id: u32, leader: String, leader_epoch: i64) -> Self {
        Self {
            id,
            leader,
            leader_epoch,
            followers: HashSet::new(),
        }
    }

    pub fn with_followers(mut self, followers: HashSet<String>) -> Self {
        self.followers = followers;
        self
    }

    pub fn is_leader(&self, address: &str) -> bool {
        self.leader == address
    }

    pub fn is_follower(&self, address: &str) -> bool {
        self.followers.contains(address)
    }
}
