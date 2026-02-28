use sofa_registry_core::model::*;
use std::collections::HashMap;

// ===========================================================================
// ConnectId tests
// ===========================================================================

#[test]
fn connect_id_new_and_fields() {
    let cid = ConnectId::new("10.0.0.1", 8080, "10.0.0.2", 9600);
    assert_eq!(cid.client_host_address, "10.0.0.1");
    assert_eq!(cid.client_port, 8080);
    assert_eq!(cid.server_host_address, "10.0.0.2");
    assert_eq!(cid.server_port, 9600);
}

#[test]
fn connect_id_display_format() {
    let cid = ConnectId::new("10.0.0.1", 8080, "10.0.0.2", 9600);
    assert_eq!(cid.to_string(), "10.0.0.1:8080-10.0.0.2:9600");
}

#[test]
fn connect_id_equality() {
    let a = ConnectId::new("10.0.0.1", 8080, "10.0.0.2", 9600);
    let b = ConnectId::new("10.0.0.1", 8080, "10.0.0.2", 9600);
    assert_eq!(a, b);
}

#[test]
fn connect_id_inequality_different_client_port() {
    let a = ConnectId::new("10.0.0.1", 8080, "10.0.0.2", 9600);
    let b = ConnectId::new("10.0.0.1", 9999, "10.0.0.2", 9600);
    assert_ne!(a, b);
}

#[test]
fn connect_id_inequality_different_server_host() {
    let a = ConnectId::new("10.0.0.1", 8080, "10.0.0.2", 9600);
    let b = ConnectId::new("10.0.0.1", 8080, "10.0.0.3", 9600);
    assert_ne!(a, b);
}

#[test]
fn connect_id_hash_is_consistent() {
    use std::collections::HashSet;
    let a = ConnectId::new("10.0.0.1", 8080, "10.0.0.2", 9600);
    let b = ConnectId::new("10.0.0.1", 8080, "10.0.0.2", 9600);
    let mut set = HashSet::new();
    set.insert(a);
    // b should be treated as the same value
    assert!(set.contains(&b));
}

#[test]
fn connect_id_serialization_roundtrip() {
    let cid = ConnectId::new("10.0.0.1", 8080, "10.0.0.2", 9600);
    let json = serde_json::to_string(&cid).unwrap();
    let deserialized: ConnectId = serde_json::from_str(&json).unwrap();
    assert_eq!(cid, deserialized);
}

// ===========================================================================
// ProcessId tests
// ===========================================================================

#[test]
fn process_id_new_and_fields() {
    let pid = ProcessId::new("192.168.1.1", 1234567890, 1);
    assert_eq!(pid.host_address, "192.168.1.1");
    assert_eq!(pid.timestamp, 1234567890);
    assert_eq!(pid.sequence_id, 1);
}

#[test]
fn process_id_display_format() {
    let pid = ProcessId::new("192.168.1.1", 1234567890, 42);
    assert_eq!(pid.to_string(), "192.168.1.1-1234567890-42");
}

#[test]
fn process_id_equality() {
    let a = ProcessId::new("10.0.0.1", 100, 1);
    let b = ProcessId::new("10.0.0.1", 100, 1);
    assert_eq!(a, b);
}

#[test]
fn process_id_inequality() {
    let a = ProcessId::new("10.0.0.1", 100, 1);
    let b = ProcessId::new("10.0.0.1", 100, 2);
    assert_ne!(a, b);
}

#[test]
fn process_id_serialization_roundtrip() {
    let pid = ProcessId::new("10.0.0.1", 1000, 5);
    let json = serde_json::to_string(&pid).unwrap();
    let deserialized: ProcessId = serde_json::from_str(&json).unwrap();
    assert_eq!(pid, deserialized);
}

// ===========================================================================
// DataBox tests
// ===========================================================================

#[test]
fn data_box_new_contains_data() {
    let db = DataBox::new("hello world");
    assert_eq!(db.data, Some("hello world".to_string()));
}

#[test]
fn data_box_empty_is_none() {
    let db = DataBox::empty();
    assert_eq!(db.data, None);
}

#[test]
fn data_box_equality() {
    let a = DataBox::new("test");
    let b = DataBox::new("test");
    assert_eq!(a, b);
}

#[test]
fn data_box_inequality() {
    let a = DataBox::new("test");
    let b = DataBox::new("other");
    assert_ne!(a, b);
}

#[test]
fn data_box_empty_not_equal_to_populated() {
    let a = DataBox::empty();
    let b = DataBox::new("data");
    assert_ne!(a, b);
}

#[test]
fn data_box_serialization_roundtrip() {
    let db = DataBox::new("payload");
    let json = serde_json::to_string(&db).unwrap();
    let deserialized: DataBox = serde_json::from_str(&json).unwrap();
    assert_eq!(db, deserialized);
}

#[test]
fn data_box_empty_serialization_roundtrip() {
    let db = DataBox::empty();
    let json = serde_json::to_string(&db).unwrap();
    let deserialized: DataBox = serde_json::from_str(&json).unwrap();
    assert_eq!(db, deserialized);
}

// ===========================================================================
// DataInfo tests
// ===========================================================================

#[test]
fn data_info_to_data_info_id() {
    let result = DataInfo::to_data_info_id(
        "com.example.Service",
        "DEFAULT_INSTANCE_ID",
        "DEFAULT_GROUP",
    );
    assert_eq!(
        result,
        "com.example.Service#DEFAULT_INSTANCE_ID#DEFAULT_GROUP"
    );
}

#[test]
fn data_info_parse_valid() {
    let result = DataInfo::parse("com.example.Service#default#DEFAULT_GROUP");
    assert_eq!(
        result,
        Some((
            "com.example.Service".to_string(),
            "default".to_string(),
            "DEFAULT_GROUP".to_string(),
        ))
    );
}

#[test]
fn data_info_parse_invalid_no_hash() {
    assert_eq!(DataInfo::parse("no-hash-here"), None);
}

#[test]
fn data_info_parse_invalid_one_hash() {
    assert_eq!(DataInfo::parse("only#one"), None);
}

#[test]
fn data_info_parse_preserves_hash_in_group() {
    // splitn(3, '#') means the group portion can contain '#'
    let result = DataInfo::parse("dataId#instanceId#group#with#hash");
    assert_eq!(
        result,
        Some((
            "dataId".to_string(),
            "instanceId".to_string(),
            "group#with#hash".to_string(),
        ))
    );
}

#[test]
fn data_info_roundtrip() {
    let data_id = "com.example.Service";
    let instance_id = "inst1";
    let group = "MY_GROUP";
    let info_id = DataInfo::to_data_info_id(data_id, instance_id, group);
    let parsed = DataInfo::parse(&info_id).unwrap();
    assert_eq!(parsed.0, data_id);
    assert_eq!(parsed.1, instance_id);
    assert_eq!(parsed.2, group);
}

#[test]
fn data_info_parse_empty_parts() {
    let result = DataInfo::parse("##");
    assert_eq!(
        result,
        Some(("".to_string(), "".to_string(), "".to_string()))
    );
}

// ===========================================================================
// EventType tests
// ===========================================================================

#[test]
fn event_type_default_is_register() {
    let et = EventType::default();
    assert_eq!(et, EventType::Register);
}

#[test]
fn event_type_display_register() {
    assert_eq!(EventType::Register.to_string(), "REGISTER");
}

#[test]
fn event_type_display_unregister() {
    assert_eq!(EventType::Unregister.to_string(), "UNREGISTER");
}

#[test]
fn event_type_serialize_register() {
    let json = serde_json::to_string(&EventType::Register).unwrap();
    assert_eq!(json, "\"REGISTER\"");
}

#[test]
fn event_type_serialize_unregister() {
    let json = serde_json::to_string(&EventType::Unregister).unwrap();
    assert_eq!(json, "\"UNREGISTER\"");
}

#[test]
fn event_type_deserialize() {
    let et: EventType = serde_json::from_str("\"REGISTER\"").unwrap();
    assert_eq!(et, EventType::Register);
    let et: EventType = serde_json::from_str("\"UNREGISTER\"").unwrap();
    assert_eq!(et, EventType::Unregister);
}

#[test]
fn event_type_equality() {
    assert_eq!(EventType::Register, EventType::Register);
    assert_eq!(EventType::Unregister, EventType::Unregister);
    assert_ne!(EventType::Register, EventType::Unregister);
}

// ===========================================================================
// Scope tests
// ===========================================================================

#[test]
fn scope_default_is_data_center() {
    let s = Scope::default();
    assert_eq!(s, Scope::DataCenter);
}

#[test]
fn scope_display_zone() {
    assert_eq!(Scope::Zone.to_string(), "zone");
}

#[test]
fn scope_display_data_center() {
    assert_eq!(Scope::DataCenter.to_string(), "dataCenter");
}

#[test]
fn scope_display_global() {
    assert_eq!(Scope::Global.to_string(), "global");
}

#[test]
fn scope_serialize() {
    assert_eq!(serde_json::to_string(&Scope::Zone).unwrap(), "\"zone\"");
    assert_eq!(
        serde_json::to_string(&Scope::DataCenter).unwrap(),
        "\"dataCenter\""
    );
    assert_eq!(serde_json::to_string(&Scope::Global).unwrap(), "\"global\"");
}

#[test]
fn scope_deserialize() {
    let z: Scope = serde_json::from_str("\"zone\"").unwrap();
    assert_eq!(z, Scope::Zone);
    let dc: Scope = serde_json::from_str("\"dataCenter\"").unwrap();
    assert_eq!(dc, Scope::DataCenter);
    let g: Scope = serde_json::from_str("\"global\"").unwrap();
    assert_eq!(g, Scope::Global);
}

#[test]
fn scope_equality() {
    assert_eq!(Scope::Zone, Scope::Zone);
    assert_ne!(Scope::Zone, Scope::DataCenter);
    assert_ne!(Scope::Zone, Scope::Global);
}

// ===========================================================================
// PublishType and PublishSource tests
// ===========================================================================

#[test]
fn publish_type_default_is_normal() {
    let pt = PublishType::default();
    assert_eq!(pt, PublishType::Normal);
}

#[test]
fn publish_type_equality() {
    assert_eq!(PublishType::Normal, PublishType::Normal);
    assert_eq!(PublishType::Temporary, PublishType::Temporary);
    assert_ne!(PublishType::Normal, PublishType::Temporary);
}

#[test]
fn publish_source_default_is_client() {
    let ps = PublishSource::default();
    assert_eq!(ps, PublishSource::Client);
}

#[test]
fn publish_source_equality() {
    assert_eq!(PublishSource::Client, PublishSource::Client);
    assert_eq!(PublishSource::SessionSync, PublishSource::SessionSync);
    assert_ne!(PublishSource::Client, PublishSource::SessionSync);
}

// ===========================================================================
// Node and NodeType tests
// ===========================================================================

#[test]
fn node_type_display() {
    assert_eq!(NodeType::Client.to_string(), "Client");
    assert_eq!(NodeType::Session.to_string(), "Session");
    assert_eq!(NodeType::Data.to_string(), "Data");
    assert_eq!(NodeType::Meta.to_string(), "Meta");
}

#[test]
fn node_new_and_fields() {
    let n = Node::new(NodeType::Session, "10.0.0.1", 9601);
    assert_eq!(n.node_type, NodeType::Session);
    assert_eq!(n.ip, "10.0.0.1");
    assert_eq!(n.port, 9601);
}

#[test]
fn node_display_format() {
    let n = Node::new(NodeType::Data, "10.0.0.2", 9621);
    assert_eq!(n.to_string(), "Data(10.0.0.2:9621)");
}

#[test]
fn node_display_format_meta() {
    let n = Node::new(NodeType::Meta, "192.168.1.100", 9611);
    assert_eq!(n.to_string(), "Meta(192.168.1.100:9611)");
}

#[test]
fn node_equality() {
    let a = Node::new(NodeType::Session, "10.0.0.1", 9601);
    let b = Node::new(NodeType::Session, "10.0.0.1", 9601);
    assert_eq!(a, b);
}

#[test]
fn node_inequality_different_type() {
    let a = Node::new(NodeType::Session, "10.0.0.1", 9601);
    let b = Node::new(NodeType::Data, "10.0.0.1", 9601);
    assert_ne!(a, b);
}

#[test]
fn node_inequality_different_port() {
    let a = Node::new(NodeType::Session, "10.0.0.1", 9601);
    let b = Node::new(NodeType::Session, "10.0.0.1", 9602);
    assert_ne!(a, b);
}

#[test]
fn node_serialization_roundtrip() {
    let n = Node::new(NodeType::Meta, "192.168.1.1", 9611);
    let json = serde_json::to_string(&n).unwrap();
    let deserialized: Node = serde_json::from_str(&json).unwrap();
    assert_eq!(n, deserialized);
}

#[test]
fn node_hash_is_consistent() {
    use std::collections::HashSet;
    let a = Node::new(NodeType::Data, "10.0.0.1", 9621);
    let b = Node::new(NodeType::Data, "10.0.0.1", 9621);
    let mut set = HashSet::new();
    set.insert(a);
    assert!(set.contains(&b));
}

// ===========================================================================
// RegisterVersion tests
// ===========================================================================

#[test]
fn register_version_new() {
    let rv = RegisterVersion::new(42, 1000);
    assert_eq!(rv.version, 42);
    assert_eq!(rv.timestamp, 1000);
}

#[test]
fn register_version_of_sets_timestamp() {
    let before = chrono::Utc::now().timestamp_millis();
    let rv = RegisterVersion::of(10);
    let after = chrono::Utc::now().timestamp_millis();
    assert_eq!(rv.version, 10);
    assert!(rv.timestamp >= before);
    assert!(rv.timestamp <= after);
}

#[test]
fn register_version_equality() {
    let a = RegisterVersion::new(1, 100);
    let b = RegisterVersion::new(1, 100);
    assert_eq!(a, b);
}

#[test]
fn register_version_inequality() {
    let a = RegisterVersion::new(1, 100);
    let b = RegisterVersion::new(1, 200);
    assert_ne!(a, b);
}

#[test]
fn register_version_serialization_roundtrip() {
    let rv = RegisterVersion::new(5, 9999);
    let json = serde_json::to_string(&rv).unwrap();
    let deserialized: RegisterVersion = serde_json::from_str(&json).unwrap();
    assert_eq!(rv, deserialized);
}

// ===========================================================================
// DatumVersion tests
// ===========================================================================

#[test]
fn datum_version_default_is_zero() {
    let dv = DatumVersion::default();
    assert_eq!(dv.value, 0);
}

#[test]
fn datum_version_of() {
    let dv = DatumVersion::of(42);
    assert_eq!(dv.value, 42);
}

#[test]
fn datum_version_next_is_current_time() {
    let before = chrono::Utc::now().timestamp_millis();
    let dv = DatumVersion::next();
    let after = chrono::Utc::now().timestamp_millis();
    assert!(dv.value >= before);
    assert!(dv.value <= after);
}

#[test]
fn datum_version_equality() {
    let a = DatumVersion::of(100);
    let b = DatumVersion::of(100);
    assert_eq!(a, b);
}

#[test]
fn datum_version_inequality() {
    let a = DatumVersion::of(100);
    let b = DatumVersion::of(200);
    assert_ne!(a, b);
}

#[test]
fn datum_version_serialization_roundtrip() {
    let dv = DatumVersion::of(123456);
    let json = serde_json::to_string(&dv).unwrap();
    let deserialized: DatumVersion = serde_json::from_str(&json).unwrap();
    assert_eq!(dv, deserialized);
}

// ===========================================================================
// Datum tests
// ===========================================================================

#[test]
fn datum_new_empty_parses_data_info_id() {
    let datum = Datum::new_empty("com.example.Service#default#DEFAULT_GROUP", "dc1");
    assert_eq!(
        datum.data_info_id,
        "com.example.Service#default#DEFAULT_GROUP"
    );
    assert_eq!(datum.data_center, "dc1");
    assert_eq!(datum.data_id, "com.example.Service");
    assert_eq!(datum.instance_id, "default");
    assert_eq!(datum.group, "DEFAULT_GROUP");
    assert!(datum.pub_map.is_empty());
    assert_eq!(datum.publisher_count(), 0);
}

#[test]
fn datum_new_empty_with_invalid_data_info_id_uses_fallback() {
    let datum = Datum::new_empty("invalid-no-hashes", "dc1");
    assert_eq!(datum.data_id, "invalid-no-hashes");
    assert_eq!(datum.instance_id, "");
    assert_eq!(datum.group, "");
}

#[test]
fn datum_publisher_count() {
    let mut datum = Datum::new_empty("svc#inst#grp", "dc1");
    assert_eq!(datum.publisher_count(), 0);

    let pub1 = Publisher {
        data_info_id: "svc#inst#grp".to_string(),
        data_id: "svc".to_string(),
        instance_id: "inst".to_string(),
        group: "grp".to_string(),
        regist_id: "reg-1".to_string(),
        client_id: "client-1".to_string(),
        cell: None,
        app_name: Some("testApp".to_string()),
        process_id: ProcessId::new("10.0.0.1", 1000, 1),
        version: RegisterVersion::new(1, 1000),
        source_address: ConnectId::new("10.0.0.1", 8080, "10.0.0.2", 9600),
        session_process_id: ProcessId::new("10.0.0.2", 2000, 1),
        data_list: vec![],
        publish_type: PublishType::Normal,
        publish_source: PublishSource::Client,
        attributes: HashMap::new(),
        register_timestamp: 1000,
    };
    datum.pub_map.insert("reg-1".to_string(), pub1);
    assert_eq!(datum.publisher_count(), 1);
}

#[test]
fn datum_version_is_set_on_creation() {
    let before = chrono::Utc::now().timestamp_millis();
    let datum = Datum::new_empty("svc#inst#grp", "dc1");
    let after = chrono::Utc::now().timestamp_millis();
    assert!(datum.version.value >= before);
    assert!(datum.version.value <= after);
}

// ===========================================================================
// RegisterResponse tests
// ===========================================================================

#[test]
fn register_response_ok() {
    let r = RegisterResponse::ok("reg-123", 5);
    assert!(r.success);
    assert_eq!(r.regist_id, Some("reg-123".to_string()));
    assert_eq!(r.version, 5);
    assert!(!r.refused);
    assert!(r.message.is_none());
}

#[test]
fn register_response_failed() {
    let r = RegisterResponse::failed("something went wrong");
    assert!(!r.success);
    assert!(r.regist_id.is_none());
    assert_eq!(r.version, 0);
    assert!(!r.refused);
    assert_eq!(r.message, Some("something went wrong".to_string()));
}

#[test]
fn register_response_refused() {
    let r = RegisterResponse::refused("access denied");
    assert!(!r.success);
    assert!(r.regist_id.is_none());
    assert_eq!(r.version, 0);
    assert!(r.refused);
    assert_eq!(r.message, Some("access denied".to_string()));
}

#[test]
fn register_response_ok_serialization_roundtrip() {
    let r = RegisterResponse::ok("reg-42", 10);
    let json = serde_json::to_string(&r).unwrap();
    let deserialized: RegisterResponse = serde_json::from_str(&json).unwrap();
    assert!(deserialized.success);
    assert_eq!(deserialized.regist_id, Some("reg-42".to_string()));
    assert_eq!(deserialized.version, 10);
    assert!(!deserialized.refused);
}

#[test]
fn register_response_failed_serialization_roundtrip() {
    let r = RegisterResponse::failed("error");
    let json = serde_json::to_string(&r).unwrap();
    let deserialized: RegisterResponse = serde_json::from_str(&json).unwrap();
    assert!(!deserialized.success);
    assert_eq!(deserialized.message, Some("error".to_string()));
}

// ===========================================================================
// ServerDataBox tests
// ===========================================================================

#[test]
fn server_data_box_new() {
    let data = bytes::Bytes::from("hello");
    let sdb = ServerDataBox::new(data.clone());
    assert_eq!(sdb.data, data);
    assert!(sdb.encoding.is_none());
}

#[test]
fn server_data_box_with_encoding() {
    let data = bytes::Bytes::from("compressed");
    let sdb = ServerDataBox::with_encoding(data.clone(), "gzip");
    assert_eq!(sdb.data, data);
    assert_eq!(sdb.encoding, Some("gzip".to_string()));
}

// ===========================================================================
// BaseRegister tests
// ===========================================================================

#[test]
fn base_register_default() {
    let br = BaseRegister::default();
    assert!(br.instance_id.is_none());
    assert!(br.zone.is_none());
    assert!(br.app_name.is_none());
    assert!(br.data_id.is_none());
    assert!(br.group.is_none());
    assert!(br.process_id.is_none());
    assert!(br.regist_id.is_none());
    assert!(br.client_id.is_none());
    assert!(br.data_info_id.is_none());
    assert!(br.ip.is_none());
    assert!(br.port.is_none());
    assert!(br.event_type.is_none());
    assert!(br.version.is_none());
    assert!(br.timestamp.is_none());
    assert!(br.attributes.is_empty());
}

#[test]
fn base_register_serialization_with_rename() {
    let br = BaseRegister {
        instance_id: Some("inst1".to_string()),
        data_id: Some("com.example.Svc".to_string()),
        app_name: Some("myApp".to_string()),
        data_info_id: Some("com.example.Svc#inst1#GRP".to_string()),
        ..Default::default()
    };

    let json = serde_json::to_string(&br).unwrap();
    // Check that serde rename fields are present
    assert!(json.contains("\"instanceId\""));
    assert!(json.contains("\"dataId\""));
    assert!(json.contains("\"appName\""));
    assert!(json.contains("\"dataInfoId\""));

    // Deserialize back
    let deserialized: BaseRegister = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.instance_id, Some("inst1".to_string()));
    assert_eq!(deserialized.data_id, Some("com.example.Svc".to_string()));
}

#[test]
fn base_register_deserialize_from_java_like_json() {
    // Simulate a JSON payload from the Java side using camelCase field names
    let json = r#"{
        "instanceId": "DEFAULT_INSTANCE_ID",
        "zone": "zone1",
        "appName": "myApp",
        "dataId": "com.example.Service",
        "group": "DEFAULT_GROUP",
        "processId": "pid-123",
        "registId": "reg-456",
        "clientId": "client-789",
        "dataInfoId": "com.example.Service#DEFAULT_INSTANCE_ID#DEFAULT_GROUP",
        "ip": "10.0.0.1",
        "port": 8080,
        "eventType": "REGISTER",
        "version": 1,
        "timestamp": 1000000
    }"#;
    let br: BaseRegister = serde_json::from_str(json).unwrap();
    assert_eq!(br.instance_id, Some("DEFAULT_INSTANCE_ID".to_string()));
    assert_eq!(br.zone, Some("zone1".to_string()));
    assert_eq!(br.app_name, Some("myApp".to_string()));
    assert_eq!(br.data_id, Some("com.example.Service".to_string()));
    assert_eq!(br.group, Some("DEFAULT_GROUP".to_string()));
    assert_eq!(br.process_id, Some("pid-123".to_string()));
    assert_eq!(br.regist_id, Some("reg-456".to_string()));
    assert_eq!(br.client_id, Some("client-789".to_string()));
    assert_eq!(br.ip, Some("10.0.0.1".to_string()));
    assert_eq!(br.port, Some(8080));
    assert_eq!(br.event_type, Some("REGISTER".to_string()));
    assert_eq!(br.version, Some(1));
    assert_eq!(br.timestamp, Some(1000000));
}

#[test]
fn base_register_attributes_map() {
    let mut br = BaseRegister::default();
    br.attributes.insert("key1".to_string(), "val1".to_string());
    br.attributes.insert("key2".to_string(), "val2".to_string());

    let json = serde_json::to_string(&br).unwrap();
    let deserialized: BaseRegister = serde_json::from_str(&json).unwrap();
    assert_eq!(
        deserialized.attributes.get("key1"),
        Some(&"val1".to_string())
    );
    assert_eq!(
        deserialized.attributes.get("key2"),
        Some(&"val2".to_string())
    );
}

// ===========================================================================
// PublisherRegister tests
// ===========================================================================

#[test]
fn publisher_register_default() {
    let pr = PublisherRegister::default();
    assert!(pr.data_list.is_empty());
    assert!(pr.base.data_id.is_none());
}

#[test]
fn publisher_register_with_data_list() {
    let pr = PublisherRegister {
        base: BaseRegister {
            data_id: Some("com.example.Svc".to_string()),
            ..Default::default()
        },
        data_list: vec![DataBox::new("payload1"), DataBox::new("payload2")],
    };
    assert_eq!(pr.data_list.len(), 2);
    assert_eq!(pr.data_list[0].data, Some("payload1".to_string()));
    assert_eq!(pr.data_list[1].data, Some("payload2".to_string()));
}

#[test]
fn publisher_register_serialization_roundtrip() {
    let pr = PublisherRegister {
        base: BaseRegister {
            data_id: Some("svc".to_string()),
            group: Some("GRP".to_string()),
            ..Default::default()
        },
        data_list: vec![DataBox::new("data")],
    };
    let json = serde_json::to_string(&pr).unwrap();
    // The flattened base fields should appear at top level
    assert!(json.contains("\"dataId\""));
    assert!(json.contains("\"dataList\""));

    let deserialized: PublisherRegister = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.base.data_id, Some("svc".to_string()));
    assert_eq!(deserialized.data_list.len(), 1);
}

// ===========================================================================
// SubscriberRegister tests
// ===========================================================================

#[test]
fn subscriber_register_default() {
    let sr = SubscriberRegister::default();
    assert_eq!(sr.scope, Scope::DataCenter);
    assert!(sr.accept_encoding.is_none());
    assert!(!sr.accept_multi);
}

#[test]
fn subscriber_register_serialization_roundtrip() {
    let sr = SubscriberRegister {
        base: BaseRegister {
            data_id: Some("com.example.Svc".to_string()),
            ..Default::default()
        },
        scope: Scope::Global,
        accept_encoding: Some("gzip".to_string()),
        accept_multi: true,
    };
    let json = serde_json::to_string(&sr).unwrap();
    assert!(json.contains("\"acceptEncoding\""));
    assert!(json.contains("\"acceptMulti\""));

    let deserialized: SubscriberRegister = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.scope, Scope::Global);
    assert_eq!(deserialized.accept_encoding, Some("gzip".to_string()));
    assert!(deserialized.accept_multi);
}

// ===========================================================================
// ReceivedData tests
// ===========================================================================

#[test]
fn received_data_serialization_roundtrip() {
    let mut data_map = HashMap::new();
    data_map.insert(
        "zone1".to_string(),
        vec![DataBox::new("payload1"), DataBox::new("payload2")],
    );

    let mut data_count = HashMap::new();
    data_count.insert("zone1".to_string(), 2u32);

    let rd = ReceivedData {
        data_id: "com.example.Svc".to_string(),
        group: "DEFAULT_GROUP".to_string(),
        instance_id: "DEFAULT_INSTANCE_ID".to_string(),
        segment: Some("seg1".to_string()),
        scope: Some("dataCenter".to_string()),
        subscriber_regist_ids: vec!["sub-1".to_string(), "sub-2".to_string()],
        data: data_map,
        version: Some(100),
        local_zone: Some("zone1".to_string()),
        data_count,
    };

    let json = serde_json::to_string(&rd).unwrap();
    assert!(json.contains("\"dataId\""));
    assert!(json.contains("\"subscriberRegistIds\""));
    assert!(json.contains("\"localZone\""));
    assert!(json.contains("\"dataCount\""));

    let deserialized: ReceivedData = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.data_id, "com.example.Svc");
    assert_eq!(deserialized.subscriber_regist_ids.len(), 2);
    assert_eq!(deserialized.data.get("zone1").unwrap().len(), 2);
    assert_eq!(deserialized.version, Some(100));
    assert_eq!(*deserialized.data_count.get("zone1").unwrap(), 2u32);
}

// ===========================================================================
// Publisher (server-side) tests
// ===========================================================================

#[test]
fn publisher_construction() {
    let pub1 = Publisher {
        data_info_id: "svc#inst#grp".to_string(),
        data_id: "svc".to_string(),
        instance_id: "inst".to_string(),
        group: "grp".to_string(),
        regist_id: "reg-1".to_string(),
        client_id: "client-1".to_string(),
        cell: Some("cell-A".to_string()),
        app_name: Some("testApp".to_string()),
        process_id: ProcessId::new("10.0.0.1", 1000, 1),
        version: RegisterVersion::new(1, 1000),
        source_address: ConnectId::new("10.0.0.1", 8080, "10.0.0.2", 9600),
        session_process_id: ProcessId::new("10.0.0.2", 2000, 1),
        data_list: vec![ServerDataBox::new(bytes::Bytes::from("data"))],
        publish_type: PublishType::Normal,
        publish_source: PublishSource::Client,
        attributes: HashMap::new(),
        register_timestamp: 1000,
    };
    assert_eq!(pub1.data_info_id, "svc#inst#grp");
    assert_eq!(pub1.data_id, "svc");
    assert_eq!(pub1.cell, Some("cell-A".to_string()));
    assert_eq!(pub1.data_list.len(), 1);
    assert_eq!(pub1.publish_type, PublishType::Normal);
    assert_eq!(pub1.publish_source, PublishSource::Client);
    assert_eq!(pub1.register_timestamp, 1000);
}

// ===========================================================================
// Subscriber (server-side) tests
// ===========================================================================

#[test]
fn subscriber_construction() {
    let sub = Subscriber {
        data_info_id: "svc#inst#grp".to_string(),
        data_id: "svc".to_string(),
        instance_id: "inst".to_string(),
        group: "grp".to_string(),
        regist_id: "sub-1".to_string(),
        client_id: "client-1".to_string(),
        scope: Scope::Global,
        cell: None,
        app_name: Some("subApp".to_string()),
        process_id: ProcessId::new("10.0.0.3", 3000, 1),
        source_address: ConnectId::new("10.0.0.3", 8080, "10.0.0.4", 9601),
        accept_encoding: Some("gzip".to_string()),
        accept_multi: true,
        register_timestamp: 2000,
    };
    assert_eq!(sub.data_info_id, "svc#inst#grp");
    assert_eq!(sub.scope, Scope::Global);
    assert!(sub.accept_multi);
    assert_eq!(sub.accept_encoding, Some("gzip".to_string()));
    assert!(sub.cell.is_none());
    assert_eq!(sub.register_timestamp, 2000);
}
