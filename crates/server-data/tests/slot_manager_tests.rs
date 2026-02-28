use sofa_registry_core::slot::{Slot, SlotTable};
use sofa_registry_server_data::slot::DataSlotManager;
use std::collections::HashSet;

const MY_ADDR: &str = "10.0.0.1:9600";
const OTHER_ADDR: &str = "10.0.0.2:9600";
const THIRD_ADDR: &str = "10.0.0.3:9600";

/// Build a simple SlotTable with the given slots.
fn build_slot_table(epoch: i64, slots: Vec<Slot>) -> SlotTable {
    SlotTable::new(epoch, slots)
}

fn slot_with_followers(id: u32, leader: &str, leader_epoch: i64, followers: Vec<&str>) -> Slot {
    let follower_set: HashSet<String> = followers.into_iter().map(String::from).collect();
    Slot::new(id, leader.to_string(), leader_epoch).with_followers(follower_set)
}

#[test]
fn new_manager_has_empty_slot_table() {
    let mgr = DataSlotManager::new(MY_ADDR);
    let table = mgr.get_slot_table();
    assert!(table.is_empty());
    assert_eq!(table.epoch, -1); // INIT_EPOCH
}

#[test]
fn new_manager_has_correct_address() {
    let mgr = DataSlotManager::new(MY_ADDR);
    assert_eq!(mgr.my_address(), MY_ADDR);
}

#[test]
fn new_manager_has_no_leader_or_follower_slots() {
    let mgr = DataSlotManager::new(MY_ADDR);
    assert!(mgr.my_leader_slots().is_empty());
    assert!(mgr.my_follower_slots().is_empty());
}

#[test]
fn update_slot_table_sets_epoch() {
    let mgr = DataSlotManager::new(MY_ADDR);
    let table = build_slot_table(42, vec![]);
    mgr.update_slot_table(table);
    assert_eq!(mgr.get_slot_table_epoch(), 42);
}

#[test]
fn update_slot_table_identifies_leader_slots() {
    let mgr = DataSlotManager::new(MY_ADDR);

    let slots = vec![
        slot_with_followers(0, MY_ADDR, 1, vec![OTHER_ADDR]),
        slot_with_followers(1, MY_ADDR, 1, vec![THIRD_ADDR]),
        slot_with_followers(2, OTHER_ADDR, 1, vec![MY_ADDR]),
    ];
    let table = build_slot_table(1, slots);
    mgr.update_slot_table(table);

    let leader_slots = mgr.my_leader_slots();
    assert_eq!(leader_slots.len(), 2);
    assert!(leader_slots.contains(&0));
    assert!(leader_slots.contains(&1));
}

#[test]
fn update_slot_table_identifies_follower_slots() {
    let mgr = DataSlotManager::new(MY_ADDR);

    let slots = vec![
        slot_with_followers(0, MY_ADDR, 1, vec![OTHER_ADDR]),
        slot_with_followers(1, OTHER_ADDR, 1, vec![MY_ADDR, THIRD_ADDR]),
        slot_with_followers(2, OTHER_ADDR, 1, vec![THIRD_ADDR]),
    ];
    let table = build_slot_table(5, slots);
    mgr.update_slot_table(table);

    let follower_slots = mgr.my_follower_slots();
    assert_eq!(follower_slots.len(), 1);
    assert!(follower_slots.contains(&1));
}

#[test]
fn am_i_leader_returns_correct_values() {
    let mgr = DataSlotManager::new(MY_ADDR);

    let slots = vec![
        slot_with_followers(0, MY_ADDR, 1, vec![OTHER_ADDR]),
        slot_with_followers(1, OTHER_ADDR, 1, vec![MY_ADDR]),
    ];
    mgr.update_slot_table(build_slot_table(1, slots));

    assert!(mgr.am_i_leader(0));
    assert!(!mgr.am_i_leader(1));
    // Non-existent slot
    assert!(!mgr.am_i_leader(999));
}

#[test]
fn am_i_follower_returns_correct_values() {
    let mgr = DataSlotManager::new(MY_ADDR);

    let slots = vec![
        slot_with_followers(0, MY_ADDR, 1, vec![OTHER_ADDR]),
        slot_with_followers(1, OTHER_ADDR, 1, vec![MY_ADDR]),
    ];
    mgr.update_slot_table(build_slot_table(1, slots));

    assert!(!mgr.am_i_follower(0));
    assert!(mgr.am_i_follower(1));
    assert!(!mgr.am_i_follower(999));
}

#[test]
fn get_leader_for_slot_returns_leader_address() {
    let mgr = DataSlotManager::new(MY_ADDR);

    let slots = vec![
        slot_with_followers(0, MY_ADDR, 1, vec![]),
        slot_with_followers(1, OTHER_ADDR, 2, vec![MY_ADDR]),
    ];
    mgr.update_slot_table(build_slot_table(10, slots));

    assert_eq!(mgr.get_leader_for_slot(0), Some(MY_ADDR.to_string()));
    assert_eq!(mgr.get_leader_for_slot(1), Some(OTHER_ADDR.to_string()));
    assert_eq!(mgr.get_leader_for_slot(999), None);
}

#[test]
fn update_slot_table_replaces_previous_table() {
    let mgr = DataSlotManager::new(MY_ADDR);

    // First update: slot 0 led by MY_ADDR
    let slots_v1 = vec![slot_with_followers(0, MY_ADDR, 1, vec![OTHER_ADDR])];
    mgr.update_slot_table(build_slot_table(1, slots_v1));
    assert!(mgr.am_i_leader(0));
    assert_eq!(mgr.get_slot_table_epoch(), 1);

    // Second update: slot 0 led by OTHER_ADDR
    let slots_v2 = vec![slot_with_followers(0, OTHER_ADDR, 2, vec![MY_ADDR])];
    mgr.update_slot_table(build_slot_table(2, slots_v2));
    assert!(!mgr.am_i_leader(0));
    assert!(mgr.am_i_follower(0));
    assert_eq!(mgr.get_slot_table_epoch(), 2);
}

#[test]
fn get_slot_table_returns_clone() {
    let mgr = DataSlotManager::new(MY_ADDR);

    let slots = vec![slot_with_followers(0, MY_ADDR, 1, vec![])];
    mgr.update_slot_table(build_slot_table(1, slots));

    let table1 = mgr.get_slot_table();
    let table2 = mgr.get_slot_table();
    assert_eq!(table1, table2);
    assert_eq!(table1.epoch, 1);
    assert_eq!(table1.slot_count(), 1);
}

#[test]
fn node_neither_leader_nor_follower_for_unrelated_slot() {
    let mgr = DataSlotManager::new(MY_ADDR);

    // Slot where MY_ADDR is not involved at all.
    let slots = vec![slot_with_followers(0, OTHER_ADDR, 1, vec![THIRD_ADDR])];
    mgr.update_slot_table(build_slot_table(1, slots));

    assert!(!mgr.am_i_leader(0));
    assert!(!mgr.am_i_follower(0));
    assert!(mgr.my_leader_slots().is_empty());
    assert!(mgr.my_follower_slots().is_empty());
}

#[test]
fn multiple_leader_and_follower_slots_mixed() {
    let mgr = DataSlotManager::new(MY_ADDR);

    let slots = vec![
        slot_with_followers(0, MY_ADDR, 1, vec![OTHER_ADDR]), // leader
        slot_with_followers(1, OTHER_ADDR, 1, vec![MY_ADDR]), // follower
        slot_with_followers(2, MY_ADDR, 1, vec![THIRD_ADDR]), // leader
        slot_with_followers(3, THIRD_ADDR, 1, vec![OTHER_ADDR]), // neither
        slot_with_followers(4, OTHER_ADDR, 1, vec![MY_ADDR, THIRD_ADDR]), // follower
    ];
    mgr.update_slot_table(build_slot_table(100, slots));

    let leaders = mgr.my_leader_slots();
    assert_eq!(leaders.len(), 2);
    assert!(leaders.contains(&0));
    assert!(leaders.contains(&2));

    let followers = mgr.my_follower_slots();
    assert_eq!(followers.len(), 2);
    assert!(followers.contains(&1));
    assert!(followers.contains(&4));
}
