use sofa_registry_core::model::{
    ConnectId, ProcessId, PublishSource, PublishType, Publisher, RegisterVersion,
};
use sofa_registry_store::memory::LocalDatumStorage;
use sofa_registry_store::traits::DatumStorage;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_publisher(data_info_id: &str, regist_id: &str, session_pid: &ProcessId) -> Publisher {
    Publisher {
        data_info_id: data_info_id.to_string(),
        data_id: "test.service".to_string(),
        instance_id: "DEFAULT_INSTANCE_ID".to_string(),
        group: "DEFAULT_GROUP".to_string(),
        regist_id: regist_id.to_string(),
        client_id: "client-1".to_string(),
        cell: None,
        app_name: Some("test-app".to_string()),
        process_id: ProcessId::new("127.0.0.1", 1000, 1),
        version: RegisterVersion::of(1),
        source_address: ConnectId::new("127.0.0.1", 12200, "127.0.0.1", 9600),
        session_process_id: session_pid.clone(),
        data_list: vec![],
        publish_type: PublishType::Normal,
        publish_source: PublishSource::Client,
        attributes: HashMap::new(),
        register_timestamp: chrono::Utc::now().timestamp_millis(),
    }
}

fn session_pid(host: &str, ts: i64) -> ProcessId {
    ProcessId::new(host, ts, 1)
}

// ---------------------------------------------------------------------------
// Basic put / get / remove
// ---------------------------------------------------------------------------

#[test]
fn put_publisher_and_get_datum() {
    let storage = LocalDatumStorage::new(256);
    let pid = session_pid("10.0.0.1", 1000);
    let pub1 = make_publisher("svc#inst#grp", "reg-1", &pid);

    let version = storage.put_publisher("dc1", pub1);
    assert!(version.value > 0, "version should be positive");

    let datum = storage.get("dc1", "svc#inst#grp").expect("datum should exist");
    assert_eq!(datum.pub_map.len(), 1);
    assert!(datum.pub_map.contains_key("reg-1"));
    assert_eq!(datum.data_info_id, "svc#inst#grp");
    assert_eq!(datum.data_center, "dc1");
}

#[test]
fn get_nonexistent_datum_returns_none() {
    let storage = LocalDatumStorage::new(256);
    assert!(storage.get("dc1", "no-such-id").is_none());
}

#[test]
fn get_nonexistent_data_center_returns_none() {
    let storage = LocalDatumStorage::new(256);
    assert!(storage.get("nonexistent-dc", "any-id").is_none());
}

#[test]
fn put_multiple_publishers_same_data_info_id() {
    let storage = LocalDatumStorage::new(256);
    let pid = session_pid("10.0.0.1", 1000);

    let pub1 = make_publisher("svc#inst#grp", "reg-1", &pid);
    let pub2 = make_publisher("svc#inst#grp", "reg-2", &pid);

    storage.put_publisher("dc1", pub1);
    storage.put_publisher("dc1", pub2);

    let datum = storage.get("dc1", "svc#inst#grp").unwrap();
    assert_eq!(datum.pub_map.len(), 2);
    assert!(datum.pub_map.contains_key("reg-1"));
    assert!(datum.pub_map.contains_key("reg-2"));
}

#[test]
fn put_publisher_overwrites_same_regist_id() {
    let storage = LocalDatumStorage::new(256);
    let pid = session_pid("10.0.0.1", 1000);

    let mut pub1 = make_publisher("svc#inst#grp", "reg-1", &pid);
    pub1.app_name = Some("old-app".to_string());
    storage.put_publisher("dc1", pub1);

    let mut pub2 = make_publisher("svc#inst#grp", "reg-1", &pid);
    pub2.app_name = Some("new-app".to_string());
    storage.put_publisher("dc1", pub2);

    let datum = storage.get("dc1", "svc#inst#grp").unwrap();
    assert_eq!(datum.pub_map.len(), 1);
    let stored = datum.pub_map.get("reg-1").unwrap();
    assert_eq!(stored.app_name.as_deref(), Some("new-app"));
}

#[test]
fn remove_publisher_success() {
    let storage = LocalDatumStorage::new(256);
    let pid = session_pid("10.0.0.1", 1000);
    let pub1 = make_publisher("svc#inst#grp", "reg-1", &pid);
    storage.put_publisher("dc1", pub1);

    let ver = storage.remove_publisher("dc1", "svc#inst#grp", "reg-1");
    assert!(ver.is_some(), "should return version on successful removal");

    let pubs = storage.get_publishers("dc1", "svc#inst#grp");
    assert!(pubs.is_empty());
}

#[test]
fn remove_nonexistent_publisher_returns_none() {
    let storage = LocalDatumStorage::new(256);
    assert!(storage.remove_publisher("dc1", "svc#inst#grp", "nope").is_none());
}

#[test]
fn remove_publisher_from_nonexistent_dc_returns_none() {
    let storage = LocalDatumStorage::new(256);
    let pid = session_pid("10.0.0.1", 1000);
    let pub1 = make_publisher("svc#inst#grp", "reg-1", &pid);
    storage.put_publisher("dc1", pub1);

    assert!(storage.remove_publisher("dc-other", "svc#inst#grp", "reg-1").is_none());
}

#[test]
fn remove_publishers_by_session_removes_correct_ones() {
    let storage = LocalDatumStorage::new(256);
    let session_a = session_pid("10.0.0.1", 1000);
    let session_b = session_pid("10.0.0.2", 2000);

    // Two publishers from session_a and one from session_b, all in the same data_info_id
    let pub1 = make_publisher("svc1#inst#grp", "reg-1", &session_a);
    let pub2 = make_publisher("svc1#inst#grp", "reg-2", &session_b);
    let pub3 = make_publisher("svc1#inst#grp", "reg-3", &session_a);

    storage.put_publisher("dc1", pub1);
    storage.put_publisher("dc1", pub2);
    storage.put_publisher("dc1", pub3);

    let updated = storage.remove_publishers_by_session("dc1", &session_a);
    assert_eq!(updated.len(), 1, "one data_info_id should be affected");
    assert!(updated.contains_key("svc1#inst#grp"));

    let pubs = storage.get_publishers("dc1", "svc1#inst#grp");
    assert_eq!(pubs.len(), 1);
    assert!(pubs.contains_key("reg-2"));
}

#[test]
fn remove_publishers_by_session_across_multiple_data_info_ids() {
    let storage = LocalDatumStorage::new(256);
    let session_a = session_pid("10.0.0.1", 1000);
    let session_b = session_pid("10.0.0.2", 2000);

    let pub1 = make_publisher("svc1#inst#grp", "reg-1", &session_a);
    let pub2 = make_publisher("svc2#inst#grp", "reg-2", &session_a);
    let pub3 = make_publisher("svc3#inst#grp", "reg-3", &session_b);

    storage.put_publisher("dc1", pub1);
    storage.put_publisher("dc1", pub2);
    storage.put_publisher("dc1", pub3);

    let updated = storage.remove_publishers_by_session("dc1", &session_a);
    assert_eq!(updated.len(), 2, "two data_info_ids should be affected");
    assert!(updated.contains_key("svc1#inst#grp"));
    assert!(updated.contains_key("svc2#inst#grp"));

    // svc3 should be untouched
    let pubs = storage.get_publishers("dc1", "svc3#inst#grp");
    assert_eq!(pubs.len(), 1);
}

#[test]
fn remove_publishers_by_session_nonexistent_dc() {
    let storage = LocalDatumStorage::new(256);
    let pid = session_pid("10.0.0.1", 1000);
    let updated = storage.remove_publishers_by_session("dc-missing", &pid);
    assert!(updated.is_empty());
}

// ---------------------------------------------------------------------------
// Version tracking
// ---------------------------------------------------------------------------

#[test]
fn get_version_after_put() {
    let storage = LocalDatumStorage::new(256);
    let pid = session_pid("10.0.0.1", 1000);

    let pub1 = make_publisher("svc#inst#grp", "reg-1", &pid);
    let put_ver = storage.put_publisher("dc1", pub1);

    let queried_ver = storage
        .get_version("dc1", "svc#inst#grp")
        .expect("version should exist");
    assert_eq!(put_ver, queried_ver);
}

#[test]
fn get_version_nonexistent_returns_none() {
    let storage = LocalDatumStorage::new(256);
    assert!(storage.get_version("dc1", "no-such").is_none());
}

#[test]
fn version_advances_on_each_put() {
    let storage = LocalDatumStorage::new(256);
    let pid = session_pid("10.0.0.1", 1000);

    let pub1 = make_publisher("svc#inst#grp", "reg-1", &pid);
    let v1 = storage.put_publisher("dc1", pub1);

    // Ensure a small delay so timestamp-based version advances
    std::thread::sleep(std::time::Duration::from_millis(2));

    let pub2 = make_publisher("svc#inst#grp", "reg-2", &pid);
    let v2 = storage.put_publisher("dc1", pub2);

    assert!(v2.value >= v1.value, "version should advance");
}

#[test]
fn version_advances_on_remove() {
    let storage = LocalDatumStorage::new(256);
    let pid = session_pid("10.0.0.1", 1000);

    let pub1 = make_publisher("svc#inst#grp", "reg-1", &pid);
    let v1 = storage.put_publisher("dc1", pub1);

    std::thread::sleep(std::time::Duration::from_millis(2));

    let v2 = storage
        .remove_publisher("dc1", "svc#inst#grp", "reg-1")
        .expect("remove should succeed");
    assert!(v2.value >= v1.value, "version should advance on remove");
}

#[test]
fn get_all_versions() {
    let storage = LocalDatumStorage::new(256);
    let pid = session_pid("10.0.0.1", 1000);

    let pub1 = make_publisher("svc1#inst#grp", "reg-1", &pid);
    let pub2 = make_publisher("svc2#inst#grp", "reg-2", &pid);
    storage.put_publisher("dc1", pub1);
    storage.put_publisher("dc1", pub2);

    let versions = storage.get_all_versions("dc1");
    assert_eq!(versions.len(), 2);
    assert!(versions.contains_key("svc1#inst#grp"));
    assert!(versions.contains_key("svc2#inst#grp"));
}

#[test]
fn get_all_versions_empty_dc() {
    let storage = LocalDatumStorage::new(256);
    let versions = storage.get_all_versions("dc-empty");
    assert!(versions.is_empty());
}

// ---------------------------------------------------------------------------
// Listing & counts
// ---------------------------------------------------------------------------

#[test]
fn get_publishers_returns_clone() {
    let storage = LocalDatumStorage::new(256);
    let pid = session_pid("10.0.0.1", 1000);

    let pub1 = make_publisher("svc#inst#grp", "reg-1", &pid);
    storage.put_publisher("dc1", pub1);

    let pubs = storage.get_publishers("dc1", "svc#inst#grp");
    assert_eq!(pubs.len(), 1);
    assert!(pubs.contains_key("reg-1"));
}

#[test]
fn get_publishers_empty_data_info_id() {
    let storage = LocalDatumStorage::new(256);
    let pubs = storage.get_publishers("dc1", "no-such");
    assert!(pubs.is_empty());
}

#[test]
fn get_all_data_info_ids() {
    let storage = LocalDatumStorage::new(256);
    let pid = session_pid("10.0.0.1", 1000);

    let pub1 = make_publisher("svc1#inst#grp", "reg-1", &pid);
    let pub2 = make_publisher("svc2#inst#grp", "reg-2", &pid);
    let pub3 = make_publisher("svc3#inst#grp", "reg-3", &pid);
    storage.put_publisher("dc1", pub1);
    storage.put_publisher("dc1", pub2);
    storage.put_publisher("dc1", pub3);

    let mut ids = storage.get_all_data_info_ids("dc1");
    ids.sort();
    assert_eq!(ids, vec!["svc1#inst#grp", "svc2#inst#grp", "svc3#inst#grp"]);
}

#[test]
fn get_all_data_info_ids_empty_dc() {
    let storage = LocalDatumStorage::new(256);
    assert!(storage.get_all_data_info_ids("dc-missing").is_empty());
}

#[test]
fn datum_count_and_publisher_count() {
    let storage = LocalDatumStorage::new(256);
    let pid = session_pid("10.0.0.1", 1000);

    assert_eq!(storage.datum_count("dc1"), 0);
    assert_eq!(storage.publisher_count("dc1"), 0);

    // Two publishers under svc1, one under svc2
    let pub1 = make_publisher("svc1#inst#grp", "reg-1", &pid);
    let pub2 = make_publisher("svc1#inst#grp", "reg-2", &pid);
    let pub3 = make_publisher("svc2#inst#grp", "reg-3", &pid);

    storage.put_publisher("dc1", pub1);
    storage.put_publisher("dc1", pub2);
    storage.put_publisher("dc1", pub3);

    assert_eq!(storage.datum_count("dc1"), 2, "two distinct data_info_ids");
    assert_eq!(storage.publisher_count("dc1"), 3, "three publishers total");
}

#[test]
fn counts_are_zero_for_nonexistent_dc() {
    let storage = LocalDatumStorage::new(256);
    assert_eq!(storage.datum_count("dc-no"), 0);
    assert_eq!(storage.publisher_count("dc-no"), 0);
}

// ---------------------------------------------------------------------------
// Slot-based operations
// ---------------------------------------------------------------------------

#[test]
fn clean_slot_removes_matching_data_info_ids() {
    let storage = LocalDatumStorage::new(256);
    let pid = session_pid("10.0.0.1", 1000);

    // Insert several publishers with different data_info_ids.
    // We will identify which slot each one maps to, and then clean that slot.
    let ids: Vec<String> = (0..20).map(|i| format!("svc-{}#inst#grp", i)).collect();

    for (i, id) in ids.iter().enumerate() {
        let pub_i = make_publisher(id, &format!("reg-{}", i), &pid);
        storage.put_publisher("dc1", pub_i);
    }

    let before_count = storage.datum_count("dc1");
    assert_eq!(before_count, 20);

    // Pick the first id, determine its slot, then clean that slot
    let target_id = &ids[0];
    let target_slot = crc32c::crc32c(target_id.as_bytes()) % 256;

    // Figure out how many ids map to the same slot (could be more than one)
    let ids_in_slot: Vec<&String> = ids
        .iter()
        .filter(|id| crc32c::crc32c(id.as_bytes()) % 256 == target_slot)
        .collect();
    assert!(
        !ids_in_slot.is_empty(),
        "at least one id should map to the target slot"
    );

    storage.clean_slot("dc1", target_slot);

    let after_count = storage.datum_count("dc1");
    assert_eq!(
        after_count,
        before_count - ids_in_slot.len(),
        "datum count should decrease by the number of ids in the cleaned slot"
    );

    // Verify the cleaned ids are gone
    for id in &ids_in_slot {
        assert!(storage.get("dc1", id).is_none(), "cleaned id should be gone");
    }

    // Verify the remaining ids are still there
    for id in &ids {
        if !ids_in_slot.contains(&id) {
            assert!(storage.get("dc1", id).is_some(), "unaffected id should remain");
        }
    }
}

#[test]
fn clean_slot_on_nonexistent_dc_does_not_panic() {
    let storage = LocalDatumStorage::new(256);
    storage.clean_slot("dc-missing", 0); // should not panic
}

// ---------------------------------------------------------------------------
// Multiple data centers
// ---------------------------------------------------------------------------

#[test]
fn data_centers_are_isolated() {
    let storage = LocalDatumStorage::new(256);
    let pid = session_pid("10.0.0.1", 1000);

    let pub1 = make_publisher("svc#inst#grp", "reg-1", &pid);
    let pub2 = make_publisher("svc#inst#grp", "reg-2", &pid);
    storage.put_publisher("dc1", pub1);
    storage.put_publisher("dc2", pub2);

    // Each DC should see only its own publisher
    let pubs_dc1 = storage.get_publishers("dc1", "svc#inst#grp");
    assert_eq!(pubs_dc1.len(), 1);
    assert!(pubs_dc1.contains_key("reg-1"));

    let pubs_dc2 = storage.get_publishers("dc2", "svc#inst#grp");
    assert_eq!(pubs_dc2.len(), 1);
    assert!(pubs_dc2.contains_key("reg-2"));

    assert_eq!(storage.datum_count("dc1"), 1);
    assert_eq!(storage.datum_count("dc2"), 1);
    assert_eq!(storage.publisher_count("dc1"), 1);
    assert_eq!(storage.publisher_count("dc2"), 1);
}

// ---------------------------------------------------------------------------
// Concurrent access
// ---------------------------------------------------------------------------

#[test]
fn concurrent_put_publishers() {
    use std::sync::Arc;
    use std::thread;

    let storage = Arc::new(LocalDatumStorage::new(256));
    let num_threads = 8;
    let pubs_per_thread = 50;

    let mut handles = vec![];
    for t in 0..num_threads {
        let storage = Arc::clone(&storage);
        handles.push(thread::spawn(move || {
            let pid = session_pid(&format!("10.0.0.{}", t), 1000 + t as i64);
            for i in 0..pubs_per_thread {
                let data_info_id = format!("svc-{}-{}#inst#grp", t, i);
                let regist_id = format!("reg-{}-{}", t, i);
                let pub_i = make_publisher(&data_info_id, &regist_id, &pid);
                storage.put_publisher("dc1", pub_i);
            }
        }));
    }

    for h in handles {
        h.join().expect("thread should not panic");
    }

    let total_datums = storage.datum_count("dc1");
    let total_pubs = storage.publisher_count("dc1");
    assert_eq!(total_datums, (num_threads * pubs_per_thread) as usize);
    assert_eq!(total_pubs, (num_threads * pubs_per_thread) as usize);
}

#[test]
fn concurrent_put_and_remove() {
    use std::sync::Arc;
    use std::thread;

    let storage = Arc::new(LocalDatumStorage::new(256));
    let pid = session_pid("10.0.0.1", 1000);

    // Pre-populate
    for i in 0..100 {
        let data_info_id = format!("svc-{}#inst#grp", i);
        let pub_i = make_publisher(&data_info_id, &format!("reg-{}", i), &pid);
        storage.put_publisher("dc1", pub_i);
    }

    let mut handles = vec![];

    // Readers
    for _ in 0..4 {
        let storage = Arc::clone(&storage);
        handles.push(thread::spawn(move || {
            for i in 0..100 {
                let _ = storage.get("dc1", &format!("svc-{}#inst#grp", i));
                let _ = storage.get_publishers("dc1", &format!("svc-{}#inst#grp", i));
            }
        }));
    }

    // Removers
    for _ in 0..2 {
        let storage = Arc::clone(&storage);
        handles.push(thread::spawn(move || {
            for i in 0..50 {
                let _ = storage.remove_publisher(
                    "dc1",
                    &format!("svc-{}#inst#grp", i),
                    &format!("reg-{}", i),
                );
            }
        }));
    }

    // Writers
    for t in 0..2 {
        let storage = Arc::clone(&storage);
        handles.push(thread::spawn(move || {
            let pid = session_pid(&format!("10.0.0.{}", t + 10), 3000);
            for i in 100..150 {
                let pub_i = make_publisher(
                    &format!("svc-{}#inst#grp", i),
                    &format!("reg-{}", i),
                    &pid,
                );
                storage.put_publisher("dc1", pub_i);
            }
        }));
    }

    for h in handles {
        h.join().expect("thread should not panic");
    }

    // Sanity check: no panic happened and we can still query
    let _ = storage.datum_count("dc1");
    let _ = storage.publisher_count("dc1");
}

#[test]
fn concurrent_put_same_data_info_id() {
    use std::sync::Arc;
    use std::thread;

    let storage = Arc::new(LocalDatumStorage::new(256));
    let num_threads = 8;
    let pubs_per_thread = 20;

    let mut handles = vec![];
    for t in 0..num_threads {
        let storage = Arc::clone(&storage);
        handles.push(thread::spawn(move || {
            let pid = session_pid(&format!("10.0.0.{}", t), 1000);
            for i in 0..pubs_per_thread {
                // All threads write to the same data_info_id but different regist_ids
                let pub_i = make_publisher(
                    "shared-svc#inst#grp",
                    &format!("reg-{}-{}", t, i),
                    &pid,
                );
                storage.put_publisher("dc1", pub_i);
            }
        }));
    }

    for h in handles {
        h.join().expect("thread should not panic");
    }

    let pubs = storage.get_publishers("dc1", "shared-svc#inst#grp");
    assert_eq!(pubs.len(), (num_threads * pubs_per_thread) as usize);
    assert_eq!(storage.datum_count("dc1"), 1);
}
