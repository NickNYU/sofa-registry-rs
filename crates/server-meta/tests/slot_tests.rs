use std::collections::HashSet;
use std::sync::Arc;

use sofa_registry_core::slot::{SlotConfig, SlotTable};
use sofa_registry_server_meta::lease::data_server_manager::{DataNode, DataServerManager};
use sofa_registry_server_meta::slot::slot_allocator::SlotAllocator;
use sofa_registry_server_meta::slot::MetaSlotManager;

// ---------------------------------------------------------------------------
// SlotAllocator::allocate - basic
// ---------------------------------------------------------------------------

#[test]
fn test_allocate_empty_servers_returns_none() {
    let servers: Vec<String> = vec![];
    assert!(SlotAllocator::allocate(256, 2, &servers, 1).is_none());
}

#[test]
fn test_allocate_single_server_all_slots_same_leader() {
    let servers = vec!["s1".to_string()];
    let table = SlotAllocator::allocate(256, 2, &servers, 1).unwrap();

    assert_eq!(table.slot_count(), 256);
    assert_eq!(table.epoch, 1);

    for slot in table.slots.values() {
        assert_eq!(slot.leader, "s1");
        // With only one server, there can be no followers
        assert!(slot.followers.is_empty());
    }
}

#[test]
fn test_allocate_two_servers_balanced() {
    let servers = vec!["a".to_string(), "b".to_string()];
    let table = SlotAllocator::allocate(256, 2, &servers, 1).unwrap();

    assert_eq!(table.slot_count(), 256);

    let a_leaders = table.get_leader_count("a");
    let b_leaders = table.get_leader_count("b");

    // 256 / 2 = 128 each
    assert_eq!(a_leaders, 128);
    assert_eq!(b_leaders, 128);
}

#[test]
fn test_allocate_three_servers_balanced() {
    let servers = vec!["s1".to_string(), "s2".to_string(), "s3".to_string()];
    let table = SlotAllocator::allocate(256, 2, &servers, 1).unwrap();

    let stats = SlotAllocator::get_distribution_stats(&table);

    // 256 / 3 = 85 or 86
    for (_server, (leader_count, _)) in &stats {
        assert!(
            *leader_count >= 85 && *leader_count <= 86,
            "Unbalanced: got {} leaders",
            leader_count
        );
    }
}

#[test]
fn test_allocate_preserves_epoch() {
    let servers = vec!["x".to_string()];
    let table = SlotAllocator::allocate(10, 1, &servers, 42).unwrap();
    assert_eq!(table.epoch, 42);
}

#[test]
fn test_allocate_slot_count_matches_requested() {
    let servers = vec!["a".to_string(), "b".to_string()];

    for num in [1, 10, 100, 256, 512] {
        let table = SlotAllocator::allocate(num, 2, &servers, 1).unwrap();
        assert_eq!(table.slot_count(), num as usize);
    }
}

// ---------------------------------------------------------------------------
// Follower allocation
// ---------------------------------------------------------------------------

#[test]
fn test_allocate_two_servers_with_replicas() {
    let servers = vec!["a".to_string(), "b".to_string()];
    let table = SlotAllocator::allocate(10, 2, &servers, 1).unwrap();

    // Each slot should have exactly 1 follower (replica=2 means 1 leader + 1 follower)
    for slot in table.slots.values() {
        assert_eq!(
            slot.followers.len(),
            1,
            "Slot {} expected 1 follower, got {}",
            slot.id,
            slot.followers.len()
        );
        // Follower should not be the same as leader
        assert!(!slot.followers.contains(&slot.leader));
    }
}

#[test]
fn test_allocate_three_servers_with_three_replicas() {
    let servers = vec!["a".to_string(), "b".to_string(), "c".to_string()];
    let table = SlotAllocator::allocate(10, 3, &servers, 1).unwrap();

    // replica=3 means 1 leader + 2 followers
    for slot in table.slots.values() {
        assert_eq!(slot.followers.len(), 2, "Slot {} expected 2 followers", slot.id);
        assert!(!slot.followers.contains(&slot.leader));
    }
}

#[test]
fn test_allocate_replicas_capped_at_server_count() {
    let servers = vec!["a".to_string(), "b".to_string()];
    // Request 5 replicas, but only 2 servers => at most 1 follower
    let table = SlotAllocator::allocate(10, 5, &servers, 1).unwrap();

    for slot in table.slots.values() {
        assert!(
            slot.followers.len() <= 1,
            "Slot {} has too many followers: {}",
            slot.id,
            slot.followers.len()
        );
        assert!(!slot.followers.contains(&slot.leader));
    }
}

#[test]
fn test_allocate_replica_one_means_no_followers() {
    let servers = vec!["a".to_string(), "b".to_string(), "c".to_string()];
    let table = SlotAllocator::allocate(10, 1, &servers, 1).unwrap();

    for slot in table.slots.values() {
        assert!(slot.followers.is_empty(), "Slot {} should have no followers", slot.id);
    }
}

// ---------------------------------------------------------------------------
// Leader uniqueness and round-robin
// ---------------------------------------------------------------------------

#[test]
fn test_allocate_round_robin_leaders() {
    let servers = vec!["s0".to_string(), "s1".to_string(), "s2".to_string()];
    let table = SlotAllocator::allocate(9, 1, &servers, 1).unwrap();

    // With 3 servers and 9 slots, round-robin gives:
    // slot 0->s0, slot 1->s1, slot 2->s2, slot 3->s0, ...
    for (slot_id, slot) in &table.slots {
        let expected_idx = *slot_id as usize % 3;
        assert_eq!(
            slot.leader,
            servers[expected_idx],
            "Slot {} expected leader {}, got {}",
            slot_id,
            servers[expected_idx],
            slot.leader
        );
    }
}

#[test]
fn test_allocate_each_slot_has_unique_id() {
    let servers = vec!["a".to_string()];
    let table = SlotAllocator::allocate(256, 1, &servers, 1).unwrap();

    let ids: HashSet<u32> = table.slots.keys().cloned().collect();
    assert_eq!(ids.len(), 256);

    // IDs should be 0..255
    for i in 0..256u32 {
        assert!(ids.contains(&i), "Missing slot id {}", i);
    }
}

// ---------------------------------------------------------------------------
// Distribution stats
// ---------------------------------------------------------------------------

#[test]
fn test_distribution_stats_single_server() {
    let servers = vec!["only".to_string()];
    let table = SlotAllocator::allocate(100, 1, &servers, 1).unwrap();

    let stats = SlotAllocator::get_distribution_stats(&table);
    assert_eq!(stats.len(), 1);
    assert_eq!(stats["only"], (100, 0));
}

#[test]
fn test_distribution_stats_leader_and_follower_counts() {
    let servers = vec!["a".to_string(), "b".to_string()];
    let table = SlotAllocator::allocate(100, 2, &servers, 1).unwrap();

    let stats = SlotAllocator::get_distribution_stats(&table);

    // Each server should lead 50 slots
    assert_eq!(stats["a"].0, 50);
    assert_eq!(stats["b"].0, 50);

    // Each server should follow the other's 50 slots
    assert_eq!(stats["a"].1, 50);
    assert_eq!(stats["b"].1, 50);
}

#[test]
fn test_distribution_stats_all_servers_present() {
    let servers: Vec<String> = (0..5).map(|i| format!("s{}", i)).collect();
    let table = SlotAllocator::allocate(256, 2, &servers, 1).unwrap();

    let stats = SlotAllocator::get_distribution_stats(&table);
    // All 5 servers should appear in stats
    assert_eq!(stats.len(), 5);
}

// ---------------------------------------------------------------------------
// Rebalance
// ---------------------------------------------------------------------------

#[test]
fn test_rebalance_empty_servers_returns_none() {
    let servers = vec!["a".to_string()];
    let table = SlotAllocator::allocate(10, 1, &servers, 1).unwrap();

    assert!(SlotAllocator::rebalance(&table, &[], 1).is_none());
}

#[test]
fn test_rebalance_same_servers_returns_none() {
    let servers = vec!["a".to_string(), "b".to_string()];
    let table = SlotAllocator::allocate(256, 2, &servers, 1).unwrap();

    // Same set of servers => no rebalance needed
    assert!(SlotAllocator::rebalance(&table, &servers, 2).is_none());
}

#[test]
fn test_rebalance_same_servers_different_order_returns_none() {
    let servers = vec!["b".to_string(), "a".to_string()];
    let table = SlotAllocator::allocate(256, 2, &vec!["a".to_string(), "b".to_string()], 1).unwrap();

    // Reversed order but same set
    assert!(SlotAllocator::rebalance(&table, &servers, 2).is_none());
}

#[test]
fn test_rebalance_add_server_increments_epoch() {
    let servers = vec!["a".to_string(), "b".to_string()];
    let table = SlotAllocator::allocate(256, 2, &servers, 1).unwrap();
    assert_eq!(table.epoch, 1);

    let new_servers = vec!["a".to_string(), "b".to_string(), "c".to_string()];
    let new_table = SlotAllocator::rebalance(&table, &new_servers, 2).unwrap();

    assert_eq!(new_table.epoch, 2);
    assert_eq!(new_table.slot_count(), 256);
}

#[test]
fn test_rebalance_add_server_includes_new_server() {
    let servers = vec!["a".to_string(), "b".to_string()];
    let table = SlotAllocator::allocate(256, 2, &servers, 1).unwrap();

    let new_servers = vec!["a".to_string(), "b".to_string(), "c".to_string()];
    let new_table = SlotAllocator::rebalance(&table, &new_servers, 2).unwrap();

    let all_servers = new_table.get_data_servers();
    assert_eq!(all_servers.len(), 3);
    assert!(all_servers.contains("c"));
}

#[test]
fn test_rebalance_remove_server() {
    let servers = vec!["a".to_string(), "b".to_string(), "c".to_string()];
    let table = SlotAllocator::allocate(256, 2, &servers, 1).unwrap();

    // Remove server c
    let new_servers = vec!["a".to_string(), "b".to_string()];
    let new_table = SlotAllocator::rebalance(&table, &new_servers, 2).unwrap();

    assert_eq!(new_table.epoch, 2);
    let all_servers = new_table.get_data_servers();
    assert!(!all_servers.contains("c"));
    assert_eq!(all_servers.len(), 2);
}

#[test]
fn test_rebalance_preserves_slot_count() {
    let servers = vec!["a".to_string(), "b".to_string()];
    let table = SlotAllocator::allocate(128, 2, &servers, 5).unwrap();

    let new_servers = vec!["a".to_string(), "b".to_string(), "c".to_string(), "d".to_string()];
    let new_table = SlotAllocator::rebalance(&table, &new_servers, 2).unwrap();

    assert_eq!(new_table.slot_count(), 128);
}

// ---------------------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------------------

#[test]
fn test_allocate_one_slot() {
    let servers = vec!["a".to_string(), "b".to_string()];
    let table = SlotAllocator::allocate(1, 2, &servers, 1).unwrap();

    assert_eq!(table.slot_count(), 1);
    let slot = table.get_slot(0).unwrap();
    assert_eq!(slot.leader, "a");
    assert_eq!(slot.followers.len(), 1);
    assert!(slot.followers.contains("b"));
}

#[test]
fn test_allocate_many_servers_few_slots() {
    // 100 servers but only 3 slots
    let servers: Vec<String> = (0..100).map(|i| format!("server-{}", i)).collect();
    let table = SlotAllocator::allocate(3, 2, &servers, 1).unwrap();

    assert_eq!(table.slot_count(), 3);

    // Only servers 0, 1, 2 should be leaders (round-robin)
    assert_eq!(table.get_slot(0).unwrap().leader, "server-0");
    assert_eq!(table.get_slot(1).unwrap().leader, "server-1");
    assert_eq!(table.get_slot(2).unwrap().leader, "server-2");
}

#[test]
fn test_allocate_large_slot_count() {
    let servers = vec!["a".to_string(), "b".to_string(), "c".to_string()];
    let table = SlotAllocator::allocate(1024, 3, &servers, 1).unwrap();

    assert_eq!(table.slot_count(), 1024);

    let stats = SlotAllocator::get_distribution_stats(&table);
    // 1024 / 3 ~= 341 or 342
    for (_server, (leader_count, _)) in &stats {
        assert!(
            *leader_count >= 341 && *leader_count <= 342,
            "Unbalanced: {} leaders",
            leader_count
        );
    }
}

// ---------------------------------------------------------------------------
// SlotTable helper methods verification
// ---------------------------------------------------------------------------

#[test]
fn test_slot_table_get_data_servers() {
    let servers = vec!["a".to_string(), "b".to_string(), "c".to_string()];
    let table = SlotAllocator::allocate(10, 2, &servers, 1).unwrap();

    let data_servers = table.get_data_servers();
    assert_eq!(data_servers.len(), 3);
    assert!(data_servers.contains("a"));
    assert!(data_servers.contains("b"));
    assert!(data_servers.contains("c"));
}

#[test]
fn test_slot_table_filter_by_server() {
    let servers = vec!["a".to_string(), "b".to_string()];
    let table = SlotAllocator::allocate(256, 2, &servers, 1).unwrap();

    let filtered = table.filter_by_server("a");
    // "a" should be involved (as leader or follower) in all 256 slots
    assert_eq!(filtered.slot_count(), 256);
}

#[test]
fn test_slot_table_is_empty() {
    let empty = SlotTable::new_empty();
    assert!(empty.is_empty());

    let servers = vec!["a".to_string()];
    let non_empty = SlotAllocator::allocate(1, 1, &servers, 1).unwrap();
    assert!(!non_empty.is_empty());
}

// ---------------------------------------------------------------------------
// MetaSlotManager integration
// ---------------------------------------------------------------------------

#[test]
fn test_slot_manager_initial_assignment() {
    let data_mgr = Arc::new(DataServerManager::new(30));
    data_mgr.register(DataNode::new("10.0.0.1:9621", "dc1", "c1"));
    data_mgr.register(DataNode::new("10.0.0.2:9621", "dc1", "c1"));

    let slot_mgr = MetaSlotManager::new(SlotConfig::default(), data_mgr);

    assert!(slot_mgr.get_slot_table().is_empty());
    assert!(slot_mgr.try_assign_or_rebalance());
    assert!(!slot_mgr.get_slot_table().is_empty());
    assert_eq!(slot_mgr.get_slot_table().slot_count(), 256);
    assert_eq!(slot_mgr.get_epoch(), 1);
}

#[test]
fn test_slot_manager_no_servers_returns_false() {
    let data_mgr = Arc::new(DataServerManager::new(30));
    let slot_mgr = MetaSlotManager::new(SlotConfig::default(), data_mgr);

    assert!(!slot_mgr.try_assign_or_rebalance());
    assert!(slot_mgr.get_slot_table().is_empty());
}

#[test]
fn test_slot_manager_rebalance_on_server_change() {
    let data_mgr = Arc::new(DataServerManager::new(30));
    data_mgr.register(DataNode::new("s1", "dc1", "c1"));
    data_mgr.register(DataNode::new("s2", "dc1", "c1"));

    let slot_mgr = MetaSlotManager::new(SlotConfig::default(), data_mgr.clone());

    // Initial assignment
    assert!(slot_mgr.try_assign_or_rebalance());
    assert_eq!(slot_mgr.get_epoch(), 1);

    // Same servers => no change
    assert!(!slot_mgr.try_assign_or_rebalance());
    assert_eq!(slot_mgr.get_epoch(), 1);

    // Add a new server => should rebalance
    data_mgr.register(DataNode::new("s3", "dc1", "c1"));
    assert!(slot_mgr.try_assign_or_rebalance());
    assert_eq!(slot_mgr.get_epoch(), 2);
}

#[test]
fn test_slot_manager_needs_rebalance() {
    let data_mgr = Arc::new(DataServerManager::new(30));
    let slot_mgr = MetaSlotManager::new(SlotConfig::default(), data_mgr.clone());

    // No servers and empty table => false (no servers to rebalance to)
    assert!(!slot_mgr.needs_rebalance());

    // Add a server but table is empty => needs initial assignment
    data_mgr.register(DataNode::new("s1", "dc1", "c1"));
    assert!(slot_mgr.needs_rebalance());

    // Perform initial assignment
    slot_mgr.try_assign_or_rebalance();

    // Same servers => no rebalance needed
    assert!(!slot_mgr.needs_rebalance());

    // Add another server => needs rebalance
    data_mgr.register(DataNode::new("s2", "dc1", "c1"));
    assert!(slot_mgr.needs_rebalance());
}

#[test]
fn test_slot_manager_set_slot_table() {
    let data_mgr = Arc::new(DataServerManager::new(30));
    let slot_mgr = MetaSlotManager::new(SlotConfig::default(), data_mgr);

    let servers = vec!["x".to_string(), "y".to_string()];
    let table = SlotAllocator::allocate(128, 2, &servers, 99).unwrap();

    slot_mgr.set_slot_table(table.clone());
    assert_eq!(slot_mgr.get_epoch(), 99);
    assert_eq!(slot_mgr.get_slot_table().slot_count(), 128);
}

#[test]
fn test_slot_manager_rebalance_after_server_removal() {
    let data_mgr = Arc::new(DataServerManager::new(30));
    data_mgr.register(DataNode::new("s1", "dc1", "c1"));
    data_mgr.register(DataNode::new("s2", "dc1", "c1"));
    data_mgr.register(DataNode::new("s3", "dc1", "c1"));

    let slot_mgr = MetaSlotManager::new(SlotConfig::default(), data_mgr.clone());

    // Initial assignment with 3 servers
    slot_mgr.try_assign_or_rebalance();
    assert_eq!(slot_mgr.get_slot_table().get_data_servers().len(), 3);

    // Remove a server
    data_mgr.remove("s3");

    // Rebalance should happen
    assert!(slot_mgr.needs_rebalance());
    assert!(slot_mgr.try_assign_or_rebalance());
    assert_eq!(slot_mgr.get_epoch(), 2);
    assert_eq!(slot_mgr.get_slot_table().get_data_servers().len(), 2);
}

// ---------------------------------------------------------------------------
// Slot balance verification for various server counts
// ---------------------------------------------------------------------------

#[test]
fn test_allocate_balance_for_various_server_counts() {
    for server_count in [2, 3, 4, 5, 7, 10, 16] {
        let servers: Vec<String> = (0..server_count).map(|i| format!("s{}", i)).collect();
        let table = SlotAllocator::allocate(256, 2, &servers, 1).unwrap();

        let stats = SlotAllocator::get_distribution_stats(&table);
        let expected_min = 256 / server_count;
        let expected_max = expected_min + 1;

        for (server, (leader_count, _)) in &stats {
            assert!(
                *leader_count >= expected_min && *leader_count <= expected_max,
                "Server {} in {}-server cluster: expected {}-{} leaders, got {}",
                server,
                server_count,
                expected_min,
                expected_max,
                leader_count
            );
        }
    }
}

#[test]
fn test_follower_never_equals_leader() {
    let servers: Vec<String> = (0..5).map(|i| format!("s{}", i)).collect();
    let table = SlotAllocator::allocate(256, 3, &servers, 1).unwrap();

    for slot in table.slots.values() {
        assert!(
            !slot.followers.contains(&slot.leader),
            "Slot {} has leader {} also as follower",
            slot.id,
            slot.leader
        );
    }
}

#[test]
fn test_all_followers_are_unique_per_slot() {
    let servers: Vec<String> = (0..5).map(|i| format!("s{}", i)).collect();
    let table = SlotAllocator::allocate(256, 4, &servers, 1).unwrap();

    for slot in table.slots.values() {
        let follower_vec: Vec<&String> = slot.followers.iter().collect();
        let follower_set: HashSet<&String> = slot.followers.iter().collect();
        assert_eq!(
            follower_vec.len(),
            follower_set.len(),
            "Slot {} has duplicate followers",
            slot.id
        );
    }
}
