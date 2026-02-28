use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

use super::Slot;

pub const INIT_EPOCH: i64 = -1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlotTable {
    pub epoch: i64,
    pub slots: BTreeMap<u32, Slot>,
}

impl SlotTable {
    pub fn new_empty() -> Self {
        Self {
            epoch: INIT_EPOCH,
            slots: BTreeMap::new(),
        }
    }

    pub fn new(epoch: i64, slots: Vec<Slot>) -> Self {
        let mut map = BTreeMap::new();
        for slot in slots {
            map.insert(slot.id, slot);
        }
        Self { epoch, slots: map }
    }

    pub fn get_slot(&self, slot_id: u32) -> Option<&Slot> {
        self.slots.get(&slot_id)
    }

    pub fn slot_of(&self, slot_id: u32) -> Option<&Slot> {
        self.slots.get(&slot_id)
    }

    /// Returns map of slot_id -> leader address
    pub fn slot_leaders(&self) -> BTreeMap<u32, String> {
        self.slots
            .iter()
            .map(|(id, s)| (*id, s.leader.clone()))
            .collect()
    }

    /// Returns all unique data server addresses involved
    pub fn get_data_servers(&self) -> BTreeSet<String> {
        let mut servers = BTreeSet::new();
        for slot in self.slots.values() {
            servers.insert(slot.leader.clone());
            for f in &slot.followers {
                servers.insert(f.clone());
            }
        }
        servers
    }

    /// Filter slots relevant to a specific data server (leader or follower)
    pub fn filter_by_server(&self, server_ip: &str) -> SlotTable {
        let filtered: BTreeMap<u32, Slot> = self
            .slots
            .iter()
            .filter(|(_, s)| s.leader == server_ip || s.followers.contains(server_ip))
            .map(|(id, s)| (*id, s.clone()))
            .collect();
        SlotTable {
            epoch: self.epoch,
            slots: filtered,
        }
    }

    /// Count how many slots the given server leads
    pub fn get_leader_count(&self, server_ip: &str) -> usize {
        self.slots
            .values()
            .filter(|s| s.leader == server_ip)
            .count()
    }

    /// Count how many slots the given server follows
    pub fn get_follower_count(&self, server_ip: &str) -> usize {
        self.slots
            .values()
            .filter(|s| s.followers.contains(server_ip))
            .count()
    }

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    pub fn slot_count(&self) -> usize {
        self.slots.len()
    }
}

impl Default for SlotTable {
    fn default() -> Self {
        Self::new_empty()
    }
}
