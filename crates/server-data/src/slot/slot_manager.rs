use parking_lot::RwLock;
use sofa_registry_core::slot::SlotTable;
use std::collections::HashSet;

/// Tracks which slots this data server is responsible for as leader or follower.
pub struct DataSlotManager {
    slot_table: RwLock<SlotTable>,
    my_address: String,
    my_leader_slots: RwLock<HashSet<u32>>,
    my_follower_slots: RwLock<HashSet<u32>>,
}

impl DataSlotManager {
    pub fn new(my_address: &str) -> Self {
        Self {
            slot_table: RwLock::new(SlotTable::new_empty()),
            my_address: my_address.to_string(),
            my_leader_slots: RwLock::new(HashSet::new()),
            my_follower_slots: RwLock::new(HashSet::new()),
        }
    }

    /// Update the local slot table and recompute leader/follower sets.
    pub fn update_slot_table(&self, table: SlotTable) {
        let mut leader_slots = HashSet::new();
        let mut follower_slots = HashSet::new();

        for (slot_id, slot) in &table.slots {
            if slot.is_leader(&self.my_address) {
                leader_slots.insert(*slot_id);
            } else if slot.is_follower(&self.my_address) {
                follower_slots.insert(*slot_id);
            }
        }

        *self.my_leader_slots.write() = leader_slots;
        *self.my_follower_slots.write() = follower_slots;
        *self.slot_table.write() = table;
    }

    pub fn get_slot_table(&self) -> SlotTable {
        self.slot_table.read().clone()
    }

    pub fn get_slot_table_epoch(&self) -> i64 {
        self.slot_table.read().epoch
    }

    pub fn am_i_leader(&self, slot_id: u32) -> bool {
        self.my_leader_slots.read().contains(&slot_id)
    }

    pub fn am_i_follower(&self, slot_id: u32) -> bool {
        self.my_follower_slots.read().contains(&slot_id)
    }

    pub fn get_leader_for_slot(&self, slot_id: u32) -> Option<String> {
        self.slot_table
            .read()
            .get_slot(slot_id)
            .map(|s| s.leader.clone())
    }

    pub fn my_leader_slots(&self) -> HashSet<u32> {
        self.my_leader_slots.read().clone()
    }

    pub fn my_follower_slots(&self) -> HashSet<u32> {
        self.my_follower_slots.read().clone()
    }

    pub fn my_address(&self) -> &str {
        &self.my_address
    }
}
