use sofa_registry_core::slot::{Slot, SlotTable};
use std::collections::{HashMap, HashSet};
use tracing::info;

/// Allocates slots across data servers using a balanced distribution algorithm.
/// Translated from Java's SlotTableBuilder/SlotAllocator.
pub struct SlotAllocator;

impl SlotAllocator {
    /// Create a new slot table with balanced distribution across data servers.
    /// Each slot gets one leader and `replicas - 1` followers.
    pub fn allocate(
        slot_num: u32,
        slot_replicas: u32,
        data_servers: &[String],
        epoch: i64,
    ) -> Option<SlotTable> {
        if data_servers.is_empty() {
            return None;
        }

        let server_count = data_servers.len();
        let follower_count = std::cmp::min(slot_replicas.saturating_sub(1) as usize, server_count.saturating_sub(1));
        let mut slots = Vec::with_capacity(slot_num as usize);

        // Distribute leaders round-robin across servers
        for slot_id in 0..slot_num {
            let leader_idx = slot_id as usize % server_count;
            let leader = data_servers[leader_idx].clone();

            // Assign followers (next servers in rotation, excluding leader)
            let mut followers = HashSet::new();
            for f in 0..follower_count {
                let follower_idx = (leader_idx + 1 + f) % server_count;
                if follower_idx != leader_idx {
                    followers.insert(data_servers[follower_idx].clone());
                }
            }

            slots.push(Slot {
                id: slot_id,
                leader,
                leader_epoch: epoch,
                followers,
            });
        }

        info!(
            "Allocated {} slots across {} servers (epoch={}, replicas={})",
            slot_num, server_count, epoch, slot_replicas
        );

        Some(SlotTable::new(epoch, slots))
    }

    /// Rebalance an existing slot table when data servers change.
    /// Returns None if no rebalancing needed (same set of servers).
    pub fn rebalance(
        current: &SlotTable,
        data_servers: &[String],
        slot_replicas: u32,
    ) -> Option<SlotTable> {
        if data_servers.is_empty() {
            return None;
        }

        // Check if the server set has changed
        let current_servers = current.get_data_servers();
        let new_servers: HashSet<String> = data_servers.iter().cloned().collect();
        
        let current_set: HashSet<&String> = current_servers.iter().collect();
        let new_set: HashSet<&String> = new_servers.iter().collect();
        
        if current_set == new_set {
            return None; // No change needed
        }

        let new_epoch = current.epoch + 1;
        
        // For simplicity, reallocate from scratch with new epoch
        // A production implementation would do minimal migration
        let mut sorted_servers: Vec<String> = data_servers.to_vec();
        sorted_servers.sort();
        
        Self::allocate(current.slot_count() as u32, slot_replicas, &sorted_servers, new_epoch)
    }

    /// Get statistics about slot distribution
    pub fn get_distribution_stats(slot_table: &SlotTable) -> HashMap<String, (usize, usize)> {
        let mut stats: HashMap<String, (usize, usize)> = HashMap::new();
        
        for slot in slot_table.slots.values() {
            let entry = stats.entry(slot.leader.clone()).or_insert((0, 0));
            entry.0 += 1; // leader count
            
            for follower in &slot.followers {
                let entry = stats.entry(follower.clone()).or_insert((0, 0));
                entry.1 += 1; // follower count
            }
        }
        
        stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allocate_single_server() {
        let servers = vec!["192.168.1.1:9621".to_string()];
        let table = SlotAllocator::allocate(256, 2, &servers, 1).unwrap();
        
        assert_eq!(table.slot_count(), 256);
        assert_eq!(table.epoch, 1);
        
        // All slots should have the same leader
        for slot in table.slots.values() {
            assert_eq!(slot.leader, "192.168.1.1:9621");
            assert!(slot.followers.is_empty()); // Can't have followers with 1 server
        }
    }

    #[test]
    fn test_allocate_three_servers() {
        let servers = vec![
            "192.168.1.1:9621".to_string(),
            "192.168.1.2:9621".to_string(),
            "192.168.1.3:9621".to_string(),
        ];
        let table = SlotAllocator::allocate(256, 2, &servers, 1).unwrap();
        
        assert_eq!(table.slot_count(), 256);
        
        let stats = SlotAllocator::get_distribution_stats(&table);
        
        // Each server should lead roughly 256/3 ≈ 85-86 slots
        for (_server, (leader_count, _)) in &stats {
            assert!(*leader_count >= 85 && *leader_count <= 86,
                "Expected ~85 leaders, got {}", leader_count);
        }
    }

    #[test]
    fn test_allocate_empty_servers() {
        let servers: Vec<String> = vec![];
        assert!(SlotAllocator::allocate(256, 2, &servers, 1).is_none());
    }

    #[test]
    fn test_rebalance_no_change() {
        let servers = vec!["s1".to_string(), "s2".to_string()];
        let table = SlotAllocator::allocate(256, 2, &servers, 1).unwrap();
        
        // Same servers - should return None
        assert!(SlotAllocator::rebalance(&table, &servers, 2).is_none());
    }

    #[test]
    fn test_rebalance_add_server() {
        let servers = vec!["s1".to_string(), "s2".to_string()];
        let table = SlotAllocator::allocate(256, 2, &servers, 1).unwrap();
        
        let new_servers = vec!["s1".to_string(), "s2".to_string(), "s3".to_string()];
        let new_table = SlotAllocator::rebalance(&table, &new_servers, 2).unwrap();
        
        assert_eq!(new_table.epoch, 2);
        assert_eq!(new_table.slot_count(), 256);
        assert_eq!(new_table.get_data_servers().len(), 3);
    }
}
