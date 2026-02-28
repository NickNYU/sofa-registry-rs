use std::collections::{BTreeMap, BTreeSet, HashSet};

use sofa_registry_core::slot::*;

// ===========================================================================
// Slot tests
// ===========================================================================

#[test]
fn slot_new_creates_with_empty_followers() {
    let slot = Slot::new(0, "10.0.0.1".to_string(), 1);
    assert_eq!(slot.id, 0);
    assert_eq!(slot.leader, "10.0.0.1");
    assert_eq!(slot.leader_epoch, 1);
    assert!(slot.followers.is_empty());
}

#[test]
fn slot_with_followers() {
    let mut followers = HashSet::new();
    followers.insert("10.0.0.2".to_string());
    followers.insert("10.0.0.3".to_string());

    let slot = Slot::new(5, "10.0.0.1".to_string(), 1).with_followers(followers.clone());
    assert_eq!(slot.followers, followers);
}

#[test]
fn slot_is_leader() {
    let slot = Slot::new(0, "10.0.0.1".to_string(), 1);
    assert!(slot.is_leader("10.0.0.1"));
    assert!(!slot.is_leader("10.0.0.2"));
}

#[test]
fn slot_is_follower() {
    let mut followers = HashSet::new();
    followers.insert("10.0.0.2".to_string());

    let slot = Slot::new(0, "10.0.0.1".to_string(), 1).with_followers(followers);
    assert!(slot.is_follower("10.0.0.2"));
    assert!(!slot.is_follower("10.0.0.3"));
    // Leader is not in the followers set
    assert!(!slot.is_follower("10.0.0.1"));
}

#[test]
fn slot_equality() {
    let mut f1 = HashSet::new();
    f1.insert("10.0.0.2".to_string());
    let mut f2 = HashSet::new();
    f2.insert("10.0.0.2".to_string());

    let a = Slot::new(0, "10.0.0.1".to_string(), 1).with_followers(f1);
    let b = Slot::new(0, "10.0.0.1".to_string(), 1).with_followers(f2);
    assert_eq!(a, b);
}

#[test]
fn slot_inequality_different_leader() {
    let a = Slot::new(0, "10.0.0.1".to_string(), 1);
    let b = Slot::new(0, "10.0.0.2".to_string(), 1);
    assert_ne!(a, b);
}

#[test]
fn slot_inequality_different_epoch() {
    let a = Slot::new(0, "10.0.0.1".to_string(), 1);
    let b = Slot::new(0, "10.0.0.1".to_string(), 2);
    assert_ne!(a, b);
}

#[test]
fn slot_serialization_roundtrip() {
    let mut followers = HashSet::new();
    followers.insert("10.0.0.2".to_string());
    followers.insert("10.0.0.3".to_string());

    let slot = Slot::new(42, "10.0.0.1".to_string(), 5).with_followers(followers);
    let json = serde_json::to_string(&slot).unwrap();
    let deserialized: Slot = serde_json::from_str(&json).unwrap();
    assert_eq!(slot, deserialized);
}

// ===========================================================================
// SlotConfig tests
// ===========================================================================

#[test]
fn slot_config_default() {
    let cfg = SlotConfig::default();
    assert_eq!(cfg.slot_num, 256);
    assert_eq!(cfg.slot_replicas, 2);
    assert_eq!(cfg.func, SlotFuncType::Crc32c);
}

#[test]
fn slot_config_serialization_roundtrip() {
    let cfg = SlotConfig::default();
    let json = serde_json::to_string(&cfg).unwrap();
    let deserialized: SlotConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.slot_num, cfg.slot_num);
    assert_eq!(deserialized.slot_replicas, cfg.slot_replicas);
    assert_eq!(deserialized.func, cfg.func);
}

#[test]
fn slot_config_custom_values() {
    let cfg = SlotConfig {
        slot_num: 512,
        slot_replicas: 3,
        func: SlotFuncType::Crc32c,
    };
    assert_eq!(cfg.slot_num, 512);
    assert_eq!(cfg.slot_replicas, 3);
}

// ===========================================================================
// SlotFuncType and SlotFunction tests
// ===========================================================================

#[test]
fn crc32c_slot_function_deterministic() {
    let func = Crc32cSlotFunction;
    let data_info_id = "com.example.Service#DEFAULT_INSTANCE_ID#DEFAULT_GROUP";
    let slot_num = 256u32;

    let s1 = func.slot_of(data_info_id, slot_num);
    let s2 = func.slot_of(data_info_id, slot_num);
    assert_eq!(s1, s2, "same input should always produce the same slot");
}

#[test]
fn crc32c_slot_function_in_range() {
    let func = Crc32cSlotFunction;
    let slot_num = 256u32;

    for i in 0..1000 {
        let id = format!("service.{}", i);
        let slot = func.slot_of(&id, slot_num);
        assert!(slot < slot_num, "slot {} should be < {}", slot, slot_num);
    }
}

#[test]
fn crc32c_slot_function_different_slot_nums() {
    let func = Crc32cSlotFunction;
    let id = "com.example.Service#inst#grp";

    // Different slot_num values should always be in range
    for &slot_num in &[1u32, 2, 16, 64, 128, 256, 512, 1024] {
        let slot = func.slot_of(id, slot_num);
        assert!(slot < slot_num, "slot {} should be < {}", slot, slot_num);
    }
}

#[test]
fn crc32c_slot_function_slot_num_1() {
    let func = Crc32cSlotFunction;
    // With only 1 slot, everything maps to slot 0
    for i in 0..100 {
        let id = format!("svc.{}", i);
        assert_eq!(func.slot_of(&id, 1), 0);
    }
}

#[test]
fn crc32c_slot_distribution_is_reasonably_uniform() {
    let func = Crc32cSlotFunction;
    let slot_num = 256u32;
    let num_keys = 10000;
    let mut counts = vec![0u32; slot_num as usize];

    for i in 0..num_keys {
        let id = format!("com.example.service.{}#DEFAULT_INSTANCE_ID#DEFAULT_GROUP", i);
        let slot = func.slot_of(&id, slot_num);
        counts[slot as usize] += 1;
    }

    // All slots should be used
    let non_empty = counts.iter().filter(|&&c| c > 0).count();
    assert!(
        non_empty > (slot_num as usize * 9) / 10,
        "at least 90% of slots should have at least one key, but only {} of {} are non-empty",
        non_empty,
        slot_num
    );

    // Check that no single slot has an extreme concentration
    let expected = num_keys as f64 / slot_num as f64; // ~39.06
    let max_count = *counts.iter().max().unwrap();
    // Allow up to 3x the expected average (very generous bound for 10K keys in 256 slots)
    assert!(
        (max_count as f64) < expected * 3.0,
        "max slot count {} is too high, expected ~{:.0}",
        max_count,
        expected
    );
}

#[test]
fn crc32c_slot_function_empty_string() {
    let func = Crc32cSlotFunction;
    // Should not panic on empty string
    let slot = func.slot_of("", 256);
    assert!(slot < 256);
}

#[test]
fn create_slot_function_crc32c() {
    let func = create_slot_function(SlotFuncType::Crc32c);
    let slot = func.slot_of("test", 256);
    assert!(slot < 256);
}

#[test]
fn create_slot_function_matches_direct_construction() {
    let factory_func = create_slot_function(SlotFuncType::Crc32c);
    let direct_func = Crc32cSlotFunction;
    let id = "com.example.Service#default#DEFAULT_GROUP";

    assert_eq!(
        factory_func.slot_of(id, 256),
        direct_func.slot_of(id, 256),
    );
}

// ===========================================================================
// SlotAccess tests
// ===========================================================================

#[test]
fn slot_access_accept() {
    let access = SlotAccess::accept(42, 10, 5);
    assert_eq!(access.slot_id, 42);
    assert_eq!(access.epoch, 10);
    assert_eq!(access.leader_epoch, 5);
    assert_eq!(access.status, SlotAccessStatus::Accept);
    assert!(access.is_accept());
    assert!(!access.is_moved());
}

#[test]
fn slot_access_moved() {
    let access = SlotAccess::moved(42, 10, 5);
    assert_eq!(access.slot_id, 42);
    assert_eq!(access.epoch, 10);
    assert_eq!(access.leader_epoch, 5);
    assert_eq!(access.status, SlotAccessStatus::Moved);
    assert!(!access.is_accept());
    assert!(access.is_moved());
}

#[test]
fn slot_access_status_values() {
    // Make sure the enum variants are distinct
    assert_ne!(SlotAccessStatus::Accept, SlotAccessStatus::Moved);
    assert_ne!(SlotAccessStatus::Accept, SlotAccessStatus::MisMatch);
    assert_ne!(SlotAccessStatus::Accept, SlotAccessStatus::Migrating);
    assert_ne!(SlotAccessStatus::Moved, SlotAccessStatus::MisMatch);
    assert_ne!(SlotAccessStatus::Moved, SlotAccessStatus::Migrating);
    assert_ne!(SlotAccessStatus::MisMatch, SlotAccessStatus::Migrating);
}

#[test]
fn slot_access_manual_construction() {
    let access = SlotAccess {
        slot_id: 100,
        status: SlotAccessStatus::MisMatch,
        epoch: 20,
        leader_epoch: 15,
    };
    assert_eq!(access.status, SlotAccessStatus::MisMatch);
    assert!(!access.is_accept());
    assert!(!access.is_moved());
}

#[test]
fn slot_access_migrating_status() {
    let access = SlotAccess {
        slot_id: 50,
        status: SlotAccessStatus::Migrating,
        epoch: 3,
        leader_epoch: 2,
    };
    assert_eq!(access.status, SlotAccessStatus::Migrating);
    assert!(!access.is_accept());
    assert!(!access.is_moved());
}

// ===========================================================================
// SlotTable tests
// ===========================================================================

#[test]
fn slot_table_new_empty() {
    let table = SlotTable::new_empty();
    assert_eq!(table.epoch, INIT_EPOCH);
    assert!(table.is_empty());
    assert_eq!(table.slot_count(), 0);
}

#[test]
fn slot_table_default_is_empty() {
    let table = SlotTable::default();
    assert_eq!(table.epoch, INIT_EPOCH);
    assert!(table.is_empty());
}

#[test]
fn slot_table_new_with_slots() {
    let slots = vec![
        Slot::new(0, "10.0.0.1".to_string(), 1),
        Slot::new(1, "10.0.0.2".to_string(), 1),
        Slot::new(2, "10.0.0.1".to_string(), 1),
    ];
    let table = SlotTable::new(5, slots);
    assert_eq!(table.epoch, 5);
    assert_eq!(table.slot_count(), 3);
    assert!(!table.is_empty());
}

#[test]
fn slot_table_get_slot() {
    let slots = vec![
        Slot::new(0, "10.0.0.1".to_string(), 1),
        Slot::new(1, "10.0.0.2".to_string(), 1),
    ];
    let table = SlotTable::new(1, slots);

    let slot0 = table.get_slot(0).unwrap();
    assert_eq!(slot0.leader, "10.0.0.1");

    let slot1 = table.get_slot(1).unwrap();
    assert_eq!(slot1.leader, "10.0.0.2");

    assert!(table.get_slot(99).is_none());
}

#[test]
fn slot_table_slot_of_alias() {
    let slots = vec![Slot::new(5, "10.0.0.1".to_string(), 1)];
    let table = SlotTable::new(1, slots);

    // slot_of is an alias for get_slot
    assert!(table.slot_of(5).is_some());
    assert!(table.slot_of(0).is_none());
}

#[test]
fn slot_table_slot_leaders() {
    let slots = vec![
        Slot::new(0, "10.0.0.1".to_string(), 1),
        Slot::new(1, "10.0.0.2".to_string(), 1),
        Slot::new(2, "10.0.0.1".to_string(), 1),
    ];
    let table = SlotTable::new(1, slots);
    let leaders = table.slot_leaders();

    let mut expected = BTreeMap::new();
    expected.insert(0, "10.0.0.1".to_string());
    expected.insert(1, "10.0.0.2".to_string());
    expected.insert(2, "10.0.0.1".to_string());

    assert_eq!(leaders, expected);
}

#[test]
fn slot_table_get_data_servers() {
    let mut f1 = HashSet::new();
    f1.insert("10.0.0.3".to_string());

    let slots = vec![
        Slot::new(0, "10.0.0.1".to_string(), 1).with_followers(f1),
        Slot::new(1, "10.0.0.2".to_string(), 1),
    ];
    let table = SlotTable::new(1, slots);
    let servers = table.get_data_servers();

    let mut expected = BTreeSet::new();
    expected.insert("10.0.0.1".to_string());
    expected.insert("10.0.0.2".to_string());
    expected.insert("10.0.0.3".to_string());

    assert_eq!(servers, expected);
}

#[test]
fn slot_table_get_data_servers_deduplication() {
    let mut f1 = HashSet::new();
    f1.insert("10.0.0.1".to_string()); // same as leader of slot 0

    let slots = vec![
        Slot::new(0, "10.0.0.1".to_string(), 1).with_followers(f1),
        Slot::new(1, "10.0.0.1".to_string(), 1),
    ];
    let table = SlotTable::new(1, slots);
    let servers = table.get_data_servers();

    // Only one unique server
    assert_eq!(servers.len(), 1);
    assert!(servers.contains("10.0.0.1"));
}

#[test]
fn slot_table_filter_by_server_leader() {
    let slots = vec![
        Slot::new(0, "10.0.0.1".to_string(), 1),
        Slot::new(1, "10.0.0.2".to_string(), 1),
        Slot::new(2, "10.0.0.1".to_string(), 1),
    ];
    let table = SlotTable::new(5, slots);
    let filtered = table.filter_by_server("10.0.0.1");

    assert_eq!(filtered.epoch, 5);
    assert_eq!(filtered.slot_count(), 2);
    assert!(filtered.get_slot(0).is_some());
    assert!(filtered.get_slot(1).is_none());
    assert!(filtered.get_slot(2).is_some());
}

#[test]
fn slot_table_filter_by_server_follower() {
    let mut f1 = HashSet::new();
    f1.insert("10.0.0.3".to_string());

    let slots = vec![
        Slot::new(0, "10.0.0.1".to_string(), 1).with_followers(f1),
        Slot::new(1, "10.0.0.2".to_string(), 1),
    ];
    let table = SlotTable::new(1, slots);
    let filtered = table.filter_by_server("10.0.0.3");

    // 10.0.0.3 is a follower of slot 0 only
    assert_eq!(filtered.slot_count(), 1);
    assert!(filtered.get_slot(0).is_some());
}

#[test]
fn slot_table_filter_by_server_no_match() {
    let slots = vec![
        Slot::new(0, "10.0.0.1".to_string(), 1),
        Slot::new(1, "10.0.0.2".to_string(), 1),
    ];
    let table = SlotTable::new(1, slots);
    let filtered = table.filter_by_server("10.0.0.99");

    assert!(filtered.is_empty());
}

#[test]
fn slot_table_get_leader_count() {
    let slots = vec![
        Slot::new(0, "10.0.0.1".to_string(), 1),
        Slot::new(1, "10.0.0.2".to_string(), 1),
        Slot::new(2, "10.0.0.1".to_string(), 1),
        Slot::new(3, "10.0.0.1".to_string(), 1),
    ];
    let table = SlotTable::new(1, slots);

    assert_eq!(table.get_leader_count("10.0.0.1"), 3);
    assert_eq!(table.get_leader_count("10.0.0.2"), 1);
    assert_eq!(table.get_leader_count("10.0.0.99"), 0);
}

#[test]
fn slot_table_get_follower_count() {
    let mut f1 = HashSet::new();
    f1.insert("10.0.0.3".to_string());
    f1.insert("10.0.0.4".to_string());

    let mut f2 = HashSet::new();
    f2.insert("10.0.0.3".to_string());

    let slots = vec![
        Slot::new(0, "10.0.0.1".to_string(), 1).with_followers(f1),
        Slot::new(1, "10.0.0.2".to_string(), 1).with_followers(f2),
    ];
    let table = SlotTable::new(1, slots);

    assert_eq!(table.get_follower_count("10.0.0.3"), 2); // follower in both slots
    assert_eq!(table.get_follower_count("10.0.0.4"), 1); // follower in slot 0 only
    assert_eq!(table.get_follower_count("10.0.0.1"), 0); // leader, not follower
    assert_eq!(table.get_follower_count("10.0.0.99"), 0);
}

#[test]
fn slot_table_equality() {
    let slots1 = vec![
        Slot::new(0, "10.0.0.1".to_string(), 1),
        Slot::new(1, "10.0.0.2".to_string(), 1),
    ];
    let slots2 = vec![
        Slot::new(0, "10.0.0.1".to_string(), 1),
        Slot::new(1, "10.0.0.2".to_string(), 1),
    ];

    let t1 = SlotTable::new(5, slots1);
    let t2 = SlotTable::new(5, slots2);
    assert_eq!(t1, t2);
}

#[test]
fn slot_table_inequality_different_epoch() {
    let slots = vec![Slot::new(0, "10.0.0.1".to_string(), 1)];
    let t1 = SlotTable::new(1, slots.clone());
    let t2 = SlotTable::new(2, slots);
    assert_ne!(t1, t2);
}

#[test]
fn slot_table_serialization_roundtrip() {
    let mut f1 = HashSet::new();
    f1.insert("10.0.0.3".to_string());

    let slots = vec![
        Slot::new(0, "10.0.0.1".to_string(), 1).with_followers(f1),
        Slot::new(1, "10.0.0.2".to_string(), 2),
    ];
    let table = SlotTable::new(10, slots);

    let json = serde_json::to_string(&table).unwrap();
    let deserialized: SlotTable = serde_json::from_str(&json).unwrap();
    assert_eq!(table, deserialized);
}

#[test]
fn slot_table_empty_serialization_roundtrip() {
    let table = SlotTable::new_empty();
    let json = serde_json::to_string(&table).unwrap();
    let deserialized: SlotTable = serde_json::from_str(&json).unwrap();
    assert_eq!(table, deserialized);
}

// ===========================================================================
// Integration: SlotFunction + SlotTable
// ===========================================================================

#[test]
fn slot_function_maps_data_info_id_to_valid_slot_in_table() {
    let func = Crc32cSlotFunction;
    let slot_num = 4u32;

    // Build a table with 4 slots
    let slots: Vec<Slot> = (0..slot_num)
        .map(|i| Slot::new(i, format!("10.0.0.{}", i + 1), 1))
        .collect();
    let table = SlotTable::new(1, slots);

    // Map a data_info_id to a slot, then look it up in the table
    let data_info_id = "com.example.Svc#inst#grp";
    let slot_id = func.slot_of(data_info_id, slot_num);
    let slot = table.get_slot(slot_id);
    assert!(slot.is_some(), "slot {} should exist in the table", slot_id);
}

#[test]
fn slot_table_large_allocation() {
    // Simulate a full 256-slot table distributed across 3 data servers
    let servers = ["10.0.0.1", "10.0.0.2", "10.0.0.3"];
    let slot_num = 256u32;

    let slots: Vec<Slot> = (0..slot_num)
        .map(|i| {
            let leader = servers[(i as usize) % servers.len()];
            let mut followers = HashSet::new();
            followers.insert(servers[((i as usize) + 1) % servers.len()].to_string());
            Slot::new(i, leader.to_string(), 1).with_followers(followers)
        })
        .collect();

    let table = SlotTable::new(1, slots);
    assert_eq!(table.slot_count(), 256);

    // Each server should lead approximately 256/3 slots
    for server in &servers {
        let count = table.get_leader_count(server);
        assert!(
            count >= 85 && count <= 86,
            "server {} leads {} slots, expected ~85",
            server,
            count
        );
    }

    // Each server should follow some slots
    for server in &servers {
        let count = table.get_follower_count(server);
        assert!(count > 0, "server {} should follow some slots", server);
    }

    // All 3 servers should be reported
    let data_servers = table.get_data_servers();
    assert_eq!(data_servers.len(), 3);
}

#[test]
fn init_epoch_constant() {
    assert_eq!(INIT_EPOCH, -1);
}
