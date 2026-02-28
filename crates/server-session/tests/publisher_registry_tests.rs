use sofa_registry_core::model::{
    ConnectId, ProcessId, PublishSource, PublishType, Publisher, RegisterVersion,
};
use sofa_registry_server_session::registry::PublisherRegistry;
use std::collections::HashMap;

/// Helper to create a Publisher with the given identifiers.
fn make_publisher(
    data_info_id: &str,
    data_id: &str,
    regist_id: &str,
    connect_id: ConnectId,
) -> Publisher {
    Publisher {
        data_info_id: data_info_id.to_string(),
        data_id: data_id.to_string(),
        instance_id: "default".to_string(),
        group: "DEFAULT_GROUP".to_string(),
        regist_id: regist_id.to_string(),
        client_id: "client-1".to_string(),
        cell: None,
        app_name: Some("test-app".to_string()),
        process_id: ProcessId::new("10.0.0.1", 1000, 1),
        version: RegisterVersion::of(1),
        source_address: connect_id,
        session_process_id: ProcessId::new("10.0.0.100", 2000, 1),
        data_list: vec![],
        publish_type: PublishType::Normal,
        publish_source: PublishSource::Client,
        attributes: HashMap::new(),
        register_timestamp: 1000,
    }
}

fn connect_id_a() -> ConnectId {
    ConnectId::new("10.0.0.1", 12345, "10.0.0.100", 9600)
}

fn connect_id_b() -> ConnectId {
    ConnectId::new("10.0.0.2", 12346, "10.0.0.100", 9600)
}

#[test]
fn new_registry_is_empty() {
    let reg = PublisherRegistry::new();
    assert_eq!(reg.count(), 0);
    assert_eq!(reg.data_info_id_count(), 0);
}

#[test]
fn default_creates_empty_registry() {
    let reg = PublisherRegistry::default();
    assert_eq!(reg.count(), 0);
}

#[test]
fn register_new_publisher_returns_true() {
    let reg = PublisherRegistry::new();
    let pub1 = make_publisher("svc.A#default#GRP", "svc.A", "reg-1", connect_id_a());
    assert!(reg.register(pub1));
}

#[test]
fn register_duplicate_regist_id_returns_false() {
    let reg = PublisherRegistry::new();
    let pub1 = make_publisher("svc.A#default#GRP", "svc.A", "reg-1", connect_id_a());
    let pub1_dup = make_publisher("svc.A#default#GRP", "svc.A", "reg-1", connect_id_a());

    assert!(reg.register(pub1));
    assert!(!reg.register(pub1_dup));
}

#[test]
fn register_increments_count() {
    let reg = PublisherRegistry::new();
    let pub1 = make_publisher("svc.A#default#GRP", "svc.A", "reg-1", connect_id_a());
    let pub2 = make_publisher("svc.A#default#GRP", "svc.A", "reg-2", connect_id_a());
    let pub3 = make_publisher("svc.B#default#GRP", "svc.B", "reg-3", connect_id_b());

    reg.register(pub1);
    reg.register(pub2);
    reg.register(pub3);

    assert_eq!(reg.count(), 3);
    assert_eq!(reg.data_info_id_count(), 2);
}

#[test]
fn get_by_data_info_id_returns_matching_publishers() {
    let reg = PublisherRegistry::new();
    let pub1 = make_publisher("svc.A#default#GRP", "svc.A", "reg-1", connect_id_a());
    let pub2 = make_publisher("svc.A#default#GRP", "svc.A", "reg-2", connect_id_b());
    let pub3 = make_publisher("svc.B#default#GRP", "svc.B", "reg-3", connect_id_a());

    reg.register(pub1);
    reg.register(pub2);
    reg.register(pub3);

    let result = reg.get_by_data_info_id("svc.A#default#GRP");
    assert_eq!(result.len(), 2);
    let regist_ids: Vec<&str> = result.iter().map(|p| p.regist_id.as_str()).collect();
    assert!(regist_ids.contains(&"reg-1"));
    assert!(regist_ids.contains(&"reg-2"));
}

#[test]
fn get_by_data_info_id_returns_empty_for_unknown() {
    let reg = PublisherRegistry::new();
    let result = reg.get_by_data_info_id("does.not.exist#default#GRP");
    assert!(result.is_empty());
}

#[test]
fn get_by_connect_id_returns_matching_publishers() {
    let reg = PublisherRegistry::new();
    let cid_a = connect_id_a();
    let pub1 = make_publisher("svc.A#default#GRP", "svc.A", "reg-1", cid_a.clone());
    let pub2 = make_publisher("svc.B#default#GRP", "svc.B", "reg-2", cid_a.clone());
    let pub3 = make_publisher("svc.C#default#GRP", "svc.C", "reg-3", connect_id_b());

    reg.register(pub1);
    reg.register(pub2);
    reg.register(pub3);

    let result = reg.get_by_connect_id(&cid_a.to_string());
    assert_eq!(result.len(), 2);
}

#[test]
fn unregister_removes_publisher() {
    let reg = PublisherRegistry::new();
    let pub1 = make_publisher("svc.A#default#GRP", "svc.A", "reg-1", connect_id_a());
    reg.register(pub1);

    let removed = reg.unregister("svc.A#default#GRP", "reg-1");
    assert!(removed.is_some());
    assert_eq!(removed.unwrap().regist_id, "reg-1");
    assert_eq!(reg.count(), 0);
    assert_eq!(reg.data_info_id_count(), 0);
}

#[test]
fn unregister_nonexistent_returns_none() {
    let reg = PublisherRegistry::new();
    assert!(reg.unregister("svc.A#default#GRP", "reg-999").is_none());
}

#[test]
fn unregister_wrong_regist_id_returns_none() {
    let reg = PublisherRegistry::new();
    let pub1 = make_publisher("svc.A#default#GRP", "svc.A", "reg-1", connect_id_a());
    reg.register(pub1);

    assert!(reg.unregister("svc.A#default#GRP", "reg-wrong").is_none());
    assert_eq!(reg.count(), 1);
}

#[test]
fn unregister_one_of_many_keeps_rest() {
    let reg = PublisherRegistry::new();
    let pub1 = make_publisher("svc.A#default#GRP", "svc.A", "reg-1", connect_id_a());
    let pub2 = make_publisher("svc.A#default#GRP", "svc.A", "reg-2", connect_id_b());
    reg.register(pub1);
    reg.register(pub2);

    let removed = reg.unregister("svc.A#default#GRP", "reg-1");
    assert!(removed.is_some());
    assert_eq!(reg.count(), 1);

    let remaining = reg.get_by_data_info_id("svc.A#default#GRP");
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].regist_id, "reg-2");
}

#[test]
fn remove_by_connect_id_removes_all_publishers_for_connection() {
    let reg = PublisherRegistry::new();
    let cid_a = connect_id_a();
    let pub1 = make_publisher("svc.A#default#GRP", "svc.A", "reg-1", cid_a.clone());
    let pub2 = make_publisher("svc.B#default#GRP", "svc.B", "reg-2", cid_a.clone());
    let pub3 = make_publisher("svc.C#default#GRP", "svc.C", "reg-3", connect_id_b());

    reg.register(pub1);
    reg.register(pub2);
    reg.register(pub3);

    let removed = reg.remove_by_connect_id(&cid_a.to_string());
    assert_eq!(removed.len(), 2);
    assert_eq!(reg.count(), 1);

    // Only pub3 should remain.
    let remaining = reg.get_by_data_info_id("svc.C#default#GRP");
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].regist_id, "reg-3");
}

#[test]
fn remove_by_connect_id_nonexistent_returns_empty() {
    let reg = PublisherRegistry::new();
    let removed = reg.remove_by_connect_id("nonexistent:0-host:0");
    assert!(removed.is_empty());
}

#[test]
fn re_register_with_same_regist_id_updates_publisher() {
    let reg = PublisherRegistry::new();
    let mut pub1 = make_publisher("svc.A#default#GRP", "svc.A", "reg-1", connect_id_a());
    pub1.register_timestamp = 100;
    reg.register(pub1);

    let mut pub1_v2 = make_publisher("svc.A#default#GRP", "svc.A", "reg-1", connect_id_a());
    pub1_v2.register_timestamp = 200;
    reg.register(pub1_v2);

    // Count should stay 1 since the duplicate was replaced.
    assert_eq!(reg.count(), 1);

    let pubs = reg.get_by_data_info_id("svc.A#default#GRP");
    assert_eq!(pubs.len(), 1);
    assert_eq!(pubs[0].register_timestamp, 200);
}

#[test]
fn unregister_cleans_up_empty_data_info_id_entry() {
    let reg = PublisherRegistry::new();
    let pub1 = make_publisher("svc.A#default#GRP", "svc.A", "reg-1", connect_id_a());
    reg.register(pub1);

    reg.unregister("svc.A#default#GRP", "reg-1");

    // After removing the only publisher, data_info_id_count should be 0.
    assert_eq!(reg.data_info_id_count(), 0);
}

#[test]
fn unregister_cleans_up_empty_connection_entry() {
    let reg = PublisherRegistry::new();
    let cid = connect_id_a();
    let pub1 = make_publisher("svc.A#default#GRP", "svc.A", "reg-1", cid.clone());
    reg.register(pub1);

    reg.unregister("svc.A#default#GRP", "reg-1");

    // After removing the only publisher for this connection, get_by_connect_id
    // should return empty.
    let by_conn = reg.get_by_connect_id(&cid.to_string());
    assert!(by_conn.is_empty());
}
