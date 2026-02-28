use sofa_registry_core::model::{ConnectId, ProcessId, Scope, Subscriber};
use sofa_registry_server_session::registry::SubscriberRegistry;

/// Helper to create a Subscriber with the given identifiers.
fn make_subscriber(
    data_info_id: &str,
    data_id: &str,
    regist_id: &str,
    connect_id: ConnectId,
) -> Subscriber {
    Subscriber {
        data_info_id: data_info_id.to_string(),
        data_id: data_id.to_string(),
        instance_id: "default".to_string(),
        group: "DEFAULT_GROUP".to_string(),
        regist_id: regist_id.to_string(),
        client_id: "client-1".to_string(),
        scope: Scope::DataCenter,
        cell: None,
        app_name: Some("test-app".to_string()),
        process_id: ProcessId::new("10.0.0.1", 1000, 1),
        source_address: connect_id,
        accept_encoding: None,
        accept_multi: false,
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
    let reg = SubscriberRegistry::new();
    assert_eq!(reg.count(), 0);
    assert_eq!(reg.data_info_id_count(), 0);
    assert!(reg.get_all_data_info_ids().is_empty());
}

#[test]
fn default_creates_empty_registry() {
    let reg = SubscriberRegistry::default();
    assert_eq!(reg.count(), 0);
}

#[test]
fn register_new_subscriber_returns_true() {
    let reg = SubscriberRegistry::new();
    let sub = make_subscriber("svc.A#default#GRP", "svc.A", "sub-1", connect_id_a());
    assert!(reg.register(sub));
}

#[test]
fn register_duplicate_regist_id_returns_false() {
    let reg = SubscriberRegistry::new();
    let sub1 = make_subscriber("svc.A#default#GRP", "svc.A", "sub-1", connect_id_a());
    let sub1_dup = make_subscriber("svc.A#default#GRP", "svc.A", "sub-1", connect_id_a());

    assert!(reg.register(sub1));
    assert!(!reg.register(sub1_dup));
}

#[test]
fn register_increments_count() {
    let reg = SubscriberRegistry::new();
    let sub1 = make_subscriber("svc.A#default#GRP", "svc.A", "sub-1", connect_id_a());
    let sub2 = make_subscriber("svc.A#default#GRP", "svc.A", "sub-2", connect_id_b());
    let sub3 = make_subscriber("svc.B#default#GRP", "svc.B", "sub-3", connect_id_a());

    reg.register(sub1);
    reg.register(sub2);
    reg.register(sub3);

    assert_eq!(reg.count(), 3);
    assert_eq!(reg.data_info_id_count(), 2);
}

#[test]
fn get_by_data_info_id_returns_matching_subscribers() {
    let reg = SubscriberRegistry::new();
    let sub1 = make_subscriber("svc.A#default#GRP", "svc.A", "sub-1", connect_id_a());
    let sub2 = make_subscriber("svc.A#default#GRP", "svc.A", "sub-2", connect_id_b());
    let sub3 = make_subscriber("svc.B#default#GRP", "svc.B", "sub-3", connect_id_a());

    reg.register(sub1);
    reg.register(sub2);
    reg.register(sub3);

    let result = reg.get_by_data_info_id("svc.A#default#GRP");
    assert_eq!(result.len(), 2);
    let regist_ids: Vec<&str> = result.iter().map(|s| s.regist_id.as_str()).collect();
    assert!(regist_ids.contains(&"sub-1"));
    assert!(regist_ids.contains(&"sub-2"));
}

#[test]
fn get_by_data_info_id_returns_empty_for_unknown() {
    let reg = SubscriberRegistry::new();
    let result = reg.get_by_data_info_id("nonexistent#default#GRP");
    assert!(result.is_empty());
}

#[test]
fn unregister_removes_subscriber() {
    let reg = SubscriberRegistry::new();
    let sub = make_subscriber("svc.A#default#GRP", "svc.A", "sub-1", connect_id_a());
    reg.register(sub);

    let removed = reg.unregister("svc.A#default#GRP", "sub-1");
    assert!(removed.is_some());
    assert_eq!(removed.unwrap().regist_id, "sub-1");
    assert_eq!(reg.count(), 0);
    assert_eq!(reg.data_info_id_count(), 0);
}

#[test]
fn unregister_nonexistent_returns_none() {
    let reg = SubscriberRegistry::new();
    assert!(reg.unregister("svc.A#default#GRP", "sub-999").is_none());
}

#[test]
fn unregister_wrong_regist_id_returns_none() {
    let reg = SubscriberRegistry::new();
    let sub = make_subscriber("svc.A#default#GRP", "svc.A", "sub-1", connect_id_a());
    reg.register(sub);

    assert!(reg.unregister("svc.A#default#GRP", "wrong-id").is_none());
    assert_eq!(reg.count(), 1);
}

#[test]
fn unregister_one_of_many_keeps_rest() {
    let reg = SubscriberRegistry::new();
    let sub1 = make_subscriber("svc.A#default#GRP", "svc.A", "sub-1", connect_id_a());
    let sub2 = make_subscriber("svc.A#default#GRP", "svc.A", "sub-2", connect_id_b());
    reg.register(sub1);
    reg.register(sub2);

    reg.unregister("svc.A#default#GRP", "sub-1");

    assert_eq!(reg.count(), 1);
    let remaining = reg.get_by_data_info_id("svc.A#default#GRP");
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].regist_id, "sub-2");
}

#[test]
fn remove_by_connect_id_removes_all_for_connection() {
    let reg = SubscriberRegistry::new();
    let cid_a = connect_id_a();
    let sub1 = make_subscriber("svc.A#default#GRP", "svc.A", "sub-1", cid_a.clone());
    let sub2 = make_subscriber("svc.B#default#GRP", "svc.B", "sub-2", cid_a.clone());
    let sub3 = make_subscriber("svc.C#default#GRP", "svc.C", "sub-3", connect_id_b());

    reg.register(sub1);
    reg.register(sub2);
    reg.register(sub3);

    let removed = reg.remove_by_connect_id(&cid_a.to_string());
    assert_eq!(removed.len(), 2);
    assert_eq!(reg.count(), 1);

    let remaining = reg.get_by_data_info_id("svc.C#default#GRP");
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].regist_id, "sub-3");
}

#[test]
fn remove_by_connect_id_nonexistent_returns_empty() {
    let reg = SubscriberRegistry::new();
    let removed = reg.remove_by_connect_id("nonexistent:0-host:0");
    assert!(removed.is_empty());
}

#[test]
fn get_all_data_info_ids_returns_all_distinct_ids() {
    let reg = SubscriberRegistry::new();
    let sub1 = make_subscriber("svc.A#default#GRP", "svc.A", "sub-1", connect_id_a());
    let sub2 = make_subscriber("svc.B#default#GRP", "svc.B", "sub-2", connect_id_a());
    let sub3 = make_subscriber("svc.B#default#GRP", "svc.B", "sub-3", connect_id_b());

    reg.register(sub1);
    reg.register(sub2);
    reg.register(sub3);

    let mut ids = reg.get_all_data_info_ids();
    ids.sort();
    assert_eq!(ids, vec!["svc.A#default#GRP", "svc.B#default#GRP"]);
}

#[test]
fn re_register_with_same_regist_id_replaces_subscriber() {
    let reg = SubscriberRegistry::new();
    let mut sub = make_subscriber("svc.A#default#GRP", "svc.A", "sub-1", connect_id_a());
    sub.register_timestamp = 100;
    reg.register(sub);

    let mut sub_v2 = make_subscriber("svc.A#default#GRP", "svc.A", "sub-1", connect_id_a());
    sub_v2.register_timestamp = 200;
    reg.register(sub_v2);

    assert_eq!(reg.count(), 1);
    let subs = reg.get_by_data_info_id("svc.A#default#GRP");
    assert_eq!(subs.len(), 1);
    assert_eq!(subs[0].register_timestamp, 200);
}

#[test]
fn unregister_cleans_up_empty_data_info_id_entry() {
    let reg = SubscriberRegistry::new();
    let sub = make_subscriber("svc.A#default#GRP", "svc.A", "sub-1", connect_id_a());
    reg.register(sub);

    reg.unregister("svc.A#default#GRP", "sub-1");
    assert_eq!(reg.data_info_id_count(), 0);
    assert!(reg.get_all_data_info_ids().is_empty());
}

#[test]
fn remove_by_connect_id_cleans_up_empty_data_info_id_entries() {
    let reg = SubscriberRegistry::new();
    let cid = connect_id_a();
    let sub1 = make_subscriber("svc.A#default#GRP", "svc.A", "sub-1", cid.clone());
    reg.register(sub1);

    reg.remove_by_connect_id(&cid.to_string());

    assert_eq!(reg.data_info_id_count(), 0);
    assert_eq!(reg.count(), 0);
}

#[test]
fn multiple_subscribers_different_scopes() {
    let reg = SubscriberRegistry::new();

    let mut sub_zone = make_subscriber("svc.A#default#GRP", "svc.A", "sub-zone", connect_id_a());
    sub_zone.scope = Scope::Zone;

    let mut sub_dc = make_subscriber("svc.A#default#GRP", "svc.A", "sub-dc", connect_id_b());
    sub_dc.scope = Scope::DataCenter;

    reg.register(sub_zone);
    reg.register(sub_dc);

    let subs = reg.get_by_data_info_id("svc.A#default#GRP");
    assert_eq!(subs.len(), 2);

    let scopes: Vec<Scope> = subs.iter().map(|s| s.scope).collect();
    assert!(scopes.contains(&Scope::Zone));
    assert!(scopes.contains(&Scope::DataCenter));
}
