use std::sync::Arc;
use parking_lot::RwLock;
use tracing::{info, warn};
use sofa_registry_core::slot::{SlotTable, SlotConfig};
use super::slot_allocator::SlotAllocator;
use crate::lease::DataServerManager;

/// Manages the global slot table in the Meta server.
/// Responsible for initial allocation and rebalancing when data servers change.
pub struct MetaSlotManager {
    slot_table: RwLock<SlotTable>,
    slot_config: SlotConfig,
    data_server_manager: Arc<DataServerManager>,
}

impl MetaSlotManager {
    pub fn new(slot_config: SlotConfig, data_server_manager: Arc<DataServerManager>) -> Self {
        Self {
            slot_table: RwLock::new(SlotTable::new_empty()),
            slot_config,
            data_server_manager,
        }
    }

    /// Get current slot table
    pub fn get_slot_table(&self) -> SlotTable {
        self.slot_table.read().clone()
    }

    /// Get current epoch
    pub fn get_epoch(&self) -> i64 {
        self.slot_table.read().epoch
    }

    /// Try to assign or rebalance slots based on current data servers.
    /// Returns true if the slot table was updated.
    pub fn try_assign_or_rebalance(&self) -> bool {
        let servers = self.data_server_manager.get_data_server_addresses();
        
        if servers.is_empty() {
            warn!("No data servers available for slot assignment");
            return false;
        }

        let current = self.slot_table.read().clone();
        
        if current.is_empty() {
            // Initial assignment
            if let Some(new_table) = SlotAllocator::allocate(
                self.slot_config.slot_num,
                self.slot_config.slot_replicas,
                &servers,
                1,
            ) {
                let stats = SlotAllocator::get_distribution_stats(&new_table);
                info!("Initial slot assignment: {} slots across {} servers", 
                    new_table.slot_count(), stats.len());
                for (server, (leaders, followers)) in &stats {
                    info!("  {} -> {} leaders, {} followers", server, leaders, followers);
                }
                *self.slot_table.write() = new_table;
                return true;
            }
        } else {
            // Rebalance
            if let Some(new_table) = SlotAllocator::rebalance(
                &current,
                &servers,
                self.slot_config.slot_replicas,
            ) {
                info!("Slot table rebalanced: epoch {} -> {}", current.epoch, new_table.epoch);
                *self.slot_table.write() = new_table;
                return true;
            }
        }

        false
    }

    /// Force set a slot table (e.g., from leader sync)
    pub fn set_slot_table(&self, table: SlotTable) {
        info!("Slot table updated to epoch {}", table.epoch);
        *self.slot_table.write() = table;
    }

    /// Check if slot table needs rebalancing
    pub fn needs_rebalance(&self) -> bool {
        let current = self.slot_table.read();
        if current.is_empty() {
            return self.data_server_manager.count() > 0;
        }
        
        let current_servers = current.get_data_servers();
        let active_servers: std::collections::BTreeSet<String> = self.data_server_manager
            .get_data_server_addresses()
            .into_iter()
            .collect();
        
        current_servers != active_servers
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lease::data_server_manager::DataNode;

    #[test]
    fn test_initial_assignment() {
        let data_mgr = Arc::new(DataServerManager::new(30));
        data_mgr.register(DataNode::new("10.0.0.1:9621", "dc1", "c1"));
        data_mgr.register(DataNode::new("10.0.0.2:9621", "dc1", "c1"));
        
        let slot_mgr = MetaSlotManager::new(SlotConfig::default(), data_mgr);
        
        assert!(slot_mgr.get_slot_table().is_empty());
        assert!(slot_mgr.try_assign_or_rebalance());
        assert!(!slot_mgr.get_slot_table().is_empty());
        assert_eq!(slot_mgr.get_slot_table().slot_count(), 256);
    }
}
