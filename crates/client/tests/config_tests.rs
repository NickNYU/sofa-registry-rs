use sofa_registry_client::RegistryClientConfig;

#[test]
fn default_config_has_expected_instance_id() {
    let cfg = RegistryClientConfig::default();
    assert_eq!(cfg.instance_id, "DEFAULT_INSTANCE_ID");
}

#[test]
fn default_config_has_expected_zone() {
    let cfg = RegistryClientConfig::default();
    assert_eq!(cfg.zone, "DEFAULT_ZONE");
}

#[test]
fn default_config_has_expected_app_name() {
    let cfg = RegistryClientConfig::default();
    assert_eq!(cfg.app_name, "default-app");
}

#[test]
fn default_config_has_expected_data_center() {
    let cfg = RegistryClientConfig::default();
    assert_eq!(cfg.data_center, "DefaultDataCenter");
}

#[test]
fn default_config_has_single_session_address() {
    let cfg = RegistryClientConfig::default();
    assert_eq!(cfg.session_server_addresses.len(), 1);
    assert_eq!(cfg.session_server_addresses[0], "127.0.0.1:9601");
}

#[test]
fn default_config_has_expected_connect_timeout() {
    let cfg = RegistryClientConfig::default();
    assert_eq!(cfg.connect_timeout_ms, 5000);
}

#[test]
fn default_config_has_expected_request_timeout() {
    let cfg = RegistryClientConfig::default();
    assert_eq!(cfg.request_timeout_ms, 10000);
}

#[test]
fn default_config_has_expected_reconnect_delay() {
    let cfg = RegistryClientConfig::default();
    assert_eq!(cfg.reconnect_delay_ms, 1000);
}

#[test]
fn default_config_has_expected_max_reconnect_delay() {
    let cfg = RegistryClientConfig::default();
    assert_eq!(cfg.max_reconnect_delay_ms, 30000);
}

#[test]
fn default_config_auth_is_disabled() {
    let cfg = RegistryClientConfig::default();
    assert!(!cfg.auth_enabled);
    assert!(cfg.access_key.is_none());
    assert!(cfg.secret_key.is_none());
}

#[test]
fn config_can_be_constructed_with_custom_values() {
    let cfg = RegistryClientConfig {
        instance_id: "my-instance".to_string(),
        zone: "us-east-1".to_string(),
        app_name: "my-service".to_string(),
        data_center: "DC1".to_string(),
        session_server_addresses: vec!["10.0.0.1:9601".to_string(), "10.0.0.2:9601".to_string()],
        connect_timeout_ms: 3000,
        request_timeout_ms: 5000,
        reconnect_delay_ms: 500,
        max_reconnect_delay_ms: 60000,
        auth_enabled: true,
        access_key: Some("ak-123".to_string()),
        secret_key: Some("sk-456".to_string()),
    };

    assert_eq!(cfg.instance_id, "my-instance");
    assert_eq!(cfg.zone, "us-east-1");
    assert_eq!(cfg.app_name, "my-service");
    assert_eq!(cfg.data_center, "DC1");
    assert_eq!(cfg.session_server_addresses.len(), 2);
    assert_eq!(cfg.session_server_addresses[0], "10.0.0.1:9601");
    assert_eq!(cfg.session_server_addresses[1], "10.0.0.2:9601");
    assert_eq!(cfg.connect_timeout_ms, 3000);
    assert_eq!(cfg.request_timeout_ms, 5000);
    assert_eq!(cfg.reconnect_delay_ms, 500);
    assert_eq!(cfg.max_reconnect_delay_ms, 60000);
    assert!(cfg.auth_enabled);
    assert_eq!(cfg.access_key.as_deref(), Some("ak-123"));
    assert_eq!(cfg.secret_key.as_deref(), Some("sk-456"));
}

#[test]
fn config_clone_produces_independent_copy() {
    let cfg1 = RegistryClientConfig {
        instance_id: "original".to_string(),
        ..Default::default()
    };
    let mut cfg2 = cfg1.clone();
    cfg2.instance_id = "clone".to_string();
    cfg2.session_server_addresses
        .push("10.0.0.3:9601".to_string());

    assert_eq!(cfg1.instance_id, "original");
    assert_eq!(cfg2.instance_id, "clone");
    assert_eq!(cfg1.session_server_addresses.len(), 1);
    assert_eq!(cfg2.session_server_addresses.len(), 2);
}

#[test]
fn config_debug_impl_works() {
    let cfg = RegistryClientConfig::default();
    let debug_str = format!("{:?}", cfg);
    assert!(debug_str.contains("RegistryClientConfig"));
    assert!(debug_str.contains("DEFAULT_INSTANCE_ID"));
}

#[test]
fn config_serializes_to_json() {
    let cfg = RegistryClientConfig::default();
    let json = serde_json::to_string(&cfg).expect("serialization should succeed");
    assert!(json.contains("\"instance_id\":\"DEFAULT_INSTANCE_ID\""));
    assert!(json.contains("\"connect_timeout_ms\":5000"));
    assert!(json.contains("\"auth_enabled\":false"));
}

#[test]
fn config_deserializes_from_json() {
    let json = r#"{
        "instance_id": "test-id",
        "zone": "test-zone",
        "app_name": "test-app",
        "data_center": "TestDC",
        "session_server_addresses": ["1.2.3.4:9601"],
        "connect_timeout_ms": 2000,
        "request_timeout_ms": 4000,
        "reconnect_delay_ms": 100,
        "max_reconnect_delay_ms": 10000,
        "auth_enabled": true,
        "access_key": "ak",
        "secret_key": "sk"
    }"#;
    let cfg: RegistryClientConfig =
        serde_json::from_str(json).expect("deserialization should succeed");
    assert_eq!(cfg.instance_id, "test-id");
    assert_eq!(cfg.zone, "test-zone");
    assert_eq!(cfg.app_name, "test-app");
    assert_eq!(cfg.data_center, "TestDC");
    assert_eq!(cfg.session_server_addresses, vec!["1.2.3.4:9601"]);
    assert_eq!(cfg.connect_timeout_ms, 2000);
    assert_eq!(cfg.request_timeout_ms, 4000);
    assert_eq!(cfg.reconnect_delay_ms, 100);
    assert_eq!(cfg.max_reconnect_delay_ms, 10000);
    assert!(cfg.auth_enabled);
    assert_eq!(cfg.access_key.as_deref(), Some("ak"));
    assert_eq!(cfg.secret_key.as_deref(), Some("sk"));
}

#[test]
fn config_roundtrip_json_serde() {
    let original = RegistryClientConfig {
        instance_id: "roundtrip".to_string(),
        zone: "zone-a".to_string(),
        app_name: "app".to_string(),
        data_center: "DC".to_string(),
        session_server_addresses: vec!["host1:1234".to_string(), "host2:5678".to_string()],
        connect_timeout_ms: 999,
        request_timeout_ms: 888,
        reconnect_delay_ms: 777,
        max_reconnect_delay_ms: 666,
        auth_enabled: false,
        access_key: None,
        secret_key: None,
    };
    let json = serde_json::to_string(&original).unwrap();
    let restored: RegistryClientConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(restored.instance_id, original.instance_id);
    assert_eq!(restored.zone, original.zone);
    assert_eq!(restored.app_name, original.app_name);
    assert_eq!(restored.data_center, original.data_center);
    assert_eq!(
        restored.session_server_addresses,
        original.session_server_addresses
    );
    assert_eq!(restored.connect_timeout_ms, original.connect_timeout_ms);
    assert_eq!(restored.request_timeout_ms, original.request_timeout_ms);
    assert_eq!(restored.reconnect_delay_ms, original.reconnect_delay_ms);
    assert_eq!(
        restored.max_reconnect_delay_ms,
        original.max_reconnect_delay_ms
    );
    assert_eq!(restored.auth_enabled, original.auth_enabled);
    assert_eq!(restored.access_key, original.access_key);
    assert_eq!(restored.secret_key, original.secret_key);
}
