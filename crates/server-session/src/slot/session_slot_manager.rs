use parking_lot::RwLock;
use sofa_registry_core::slot::SlotTable;
use sofa_registry_core::slot::{Crc32cSlotFunction, SlotFunction};

/// Caches the cluster slot table for routing writes to the correct data server.
pub struct SessionSlotManager {
    slot_table: RwLock<SlotTable>,
    slot_func: Crc32cSlotFunction,
    slot_num: u32,
}

impl SessionSlotManager {
    pub fn new(slot_num: u32) -> Self {
        Self {
            slot_table: RwLock::new(SlotTable::new_empty()),
            slot_func: Crc32cSlotFunction,
            slot_num,
        }
    }

    /// Update the cached slot table.
    pub fn update_slot_table(&self, table: SlotTable) {
        *self.slot_table.write() = table;
    }

    /// Get the current slot table epoch.
    pub fn get_epoch(&self) -> i64 {
        self.slot_table.read().epoch
    }

    /// Calculate the slot id for a given data_info_id.
    pub fn slot_of(&self, data_info_id: &str) -> u32 {
        self.slot_func.slot_of(data_info_id, self.slot_num)
    }

    /// Get the leader address for a given slot.
    pub fn get_leader_for_slot(&self, slot_id: u32) -> Option<String> {
        self.slot_table
            .read()
            .get_slot(slot_id)
            .map(|s| s.leader.clone())
    }

    /// Get the leader address for a given data_info_id.
    /// Returns (slot_id, leader_address) or None if no leader is known.
    pub fn get_leader_for_data(&self, data_info_id: &str) -> Option<(u32, String)> {
        let slot_id = self.slot_of(data_info_id);
        let leader = self.get_leader_for_slot(slot_id)?;
        if leader.is_empty() {
            return None;
        }
        Some((slot_id, leader))
    }

    /// Get the slot table.
    pub fn get_slot_table(&self) -> SlotTable {
        self.slot_table.read().clone()
    }

    /// Check if the slot table has been initialized (epoch > 0).
    pub fn is_initialized(&self) -> bool {
        self.slot_table.read().epoch > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sofa_registry_core::slot::Slot;
    use std::collections::HashSet;

    fn make_test_slot_table() -> SlotTable {
        let slots = vec![
            Slot::new(0, "10.0.0.1:9621".to_string(), 1)
                .with_followers(HashSet::from(["10.0.0.2:9621".to_string()])),
            Slot::new(1, "10.0.0.2:9621".to_string(), 1)
                .with_followers(HashSet::from(["10.0.0.1:9621".to_string()])),
            Slot::new(2, "10.0.0.1:9621".to_string(), 1),
        ];
        SlotTable::new(5, slots)
    }

    #[test]
    fn test_new_manager_is_not_initialized() {
        let mgr = SessionSlotManager::new(256);
        assert!(!mgr.is_initialized());
        assert_eq!(mgr.get_epoch(), -1);
    }

    #[test]
    fn test_update_and_get_epoch() {
        let mgr = SessionSlotManager::new(256);
        let table = make_test_slot_table();
        mgr.update_slot_table(table);
        assert_eq!(mgr.get_epoch(), 5);
        assert!(mgr.is_initialized());
    }

    #[test]
    fn test_get_leader_for_slot() {
        let mgr = SessionSlotManager::new(256);
        mgr.update_slot_table(make_test_slot_table());

        assert_eq!(
            mgr.get_leader_for_slot(0),
            Some("10.0.0.1:9621".to_string())
        );
        assert_eq!(
            mgr.get_leader_for_slot(1),
            Some("10.0.0.2:9621".to_string())
        );
        assert_eq!(mgr.get_leader_for_slot(999), None);
    }

    #[test]
    fn test_get_leader_for_data() {
        let mgr = SessionSlotManager::new(3); // only 3 slots for testability
        mgr.update_slot_table(make_test_slot_table());

        // slot_of should deterministically map to a slot 0..3
        let result = mgr.get_leader_for_data("com.example.service");
        assert!(result.is_some());
        let (slot_id, leader) = result.unwrap();
        assert!(slot_id < 3);
        assert!(!leader.is_empty());
    }

    #[test]
    fn test_get_leader_for_data_returns_none_when_empty() {
        let mgr = SessionSlotManager::new(256);
        // No slot table set, so no leaders known
        assert!(mgr.get_leader_for_data("com.example.service").is_none());
    }

    #[test]
    fn test_get_slot_table_clone() {
        let mgr = SessionSlotManager::new(256);
        let table = make_test_slot_table();
        mgr.update_slot_table(table.clone());
        let retrieved = mgr.get_slot_table();
        assert_eq!(retrieved.epoch, table.epoch);
        assert_eq!(retrieved.slot_count(), table.slot_count());
    }

    #[test]
    fn test_slot_of_deterministic() {
        let mgr = SessionSlotManager::new(256);
        let s1 = mgr.slot_of("com.example.service#DEFAULT_INSTANCE_ID#DEFAULT_GROUP");
        let s2 = mgr.slot_of("com.example.service#DEFAULT_INSTANCE_ID#DEFAULT_GROUP");
        assert_eq!(s1, s2);
        assert!(s1 < 256);
    }
}
