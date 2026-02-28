use sofa_registry_client::{PublisherRegistration, SubscriberRegistration};
use sofa_registry_core::model::Scope;

// ---------------------------------------------------------------------------
// PublisherRegistration tests
// ---------------------------------------------------------------------------

#[test]
fn publisher_new_sets_data_id() {
    let reg = PublisherRegistration::new("com.example.MyService");
    assert_eq!(reg.data_id, "com.example.MyService");
}

#[test]
fn publisher_new_defaults_group() {
    let reg = PublisherRegistration::new("svc");
    assert_eq!(reg.group, "DEFAULT_GROUP");
}

#[test]
fn publisher_new_defaults_instance_id() {
    let reg = PublisherRegistration::new("svc");
    assert_eq!(reg.instance_id, "DEFAULT_INSTANCE_ID");
}

#[test]
fn publisher_new_defaults_app_name_to_none() {
    let reg = PublisherRegistration::new("svc");
    assert!(reg.app_name.is_none());
}

#[test]
fn publisher_with_group_overrides_default() {
    let reg = PublisherRegistration::new("svc").with_group("my-group");
    assert_eq!(reg.group, "my-group");
}

#[test]
fn publisher_with_instance_id_overrides_default() {
    let reg = PublisherRegistration::new("svc").with_instance_id("inst-001");
    assert_eq!(reg.instance_id, "inst-001");
}

#[test]
fn publisher_with_app_name_sets_value() {
    let reg = PublisherRegistration::new("svc").with_app_name("my-app");
    assert_eq!(reg.app_name.as_deref(), Some("my-app"));
}

#[test]
fn publisher_builder_chain() {
    let reg = PublisherRegistration::new("com.example.Service")
        .with_group("grp")
        .with_instance_id("inst")
        .with_app_name("app");
    assert_eq!(reg.data_id, "com.example.Service");
    assert_eq!(reg.group, "grp");
    assert_eq!(reg.instance_id, "inst");
    assert_eq!(reg.app_name.as_deref(), Some("app"));
}

#[test]
fn publisher_data_info_id_default_format() {
    let reg = PublisherRegistration::new("com.example.Svc");
    assert_eq!(
        reg.data_info_id(),
        "com.example.Svc#DEFAULT_INSTANCE_ID#DEFAULT_GROUP"
    );
}

#[test]
fn publisher_data_info_id_with_custom_values() {
    let reg = PublisherRegistration::new("svc")
        .with_instance_id("inst-1")
        .with_group("grp-1");
    assert_eq!(reg.data_info_id(), "svc#inst-1#grp-1");
}

#[test]
fn publisher_new_accepts_string() {
    let reg = PublisherRegistration::new(String::from("owned-string-svc"));
    assert_eq!(reg.data_id, "owned-string-svc");
}

#[test]
fn publisher_clone_is_independent() {
    let reg1 = PublisherRegistration::new("svc").with_group("g1");
    let mut reg2 = reg1.clone();
    reg2.group = "g2".to_string();
    assert_eq!(reg1.group, "g1");
    assert_eq!(reg2.group, "g2");
}

#[test]
fn publisher_debug_impl() {
    let reg = PublisherRegistration::new("svc");
    let dbg = format!("{:?}", reg);
    assert!(dbg.contains("PublisherRegistration"));
    assert!(dbg.contains("svc"));
}

// ---------------------------------------------------------------------------
// SubscriberRegistration tests
// ---------------------------------------------------------------------------

#[test]
fn subscriber_new_sets_data_id() {
    let reg = SubscriberRegistration::new("com.example.MyService");
    assert_eq!(reg.data_id, "com.example.MyService");
}

#[test]
fn subscriber_new_defaults_group() {
    let reg = SubscriberRegistration::new("svc");
    assert_eq!(reg.group, "DEFAULT_GROUP");
}

#[test]
fn subscriber_new_defaults_instance_id() {
    let reg = SubscriberRegistration::new("svc");
    assert_eq!(reg.instance_id, "DEFAULT_INSTANCE_ID");
}

#[test]
fn subscriber_new_defaults_scope_to_datacenter() {
    let reg = SubscriberRegistration::new("svc");
    assert_eq!(reg.scope, Scope::DataCenter);
}

#[test]
fn subscriber_new_defaults_app_name_to_none() {
    let reg = SubscriberRegistration::new("svc");
    assert!(reg.app_name.is_none());
}

#[test]
fn subscriber_with_group_overrides_default() {
    let reg = SubscriberRegistration::new("svc").with_group("custom-group");
    assert_eq!(reg.group, "custom-group");
}

#[test]
fn subscriber_with_instance_id_overrides_default() {
    let reg = SubscriberRegistration::new("svc").with_instance_id("inst-002");
    assert_eq!(reg.instance_id, "inst-002");
}

#[test]
fn subscriber_with_scope_zone() {
    let reg = SubscriberRegistration::new("svc").with_scope(Scope::Zone);
    assert_eq!(reg.scope, Scope::Zone);
}

#[test]
fn subscriber_with_scope_global() {
    let reg = SubscriberRegistration::new("svc").with_scope(Scope::Global);
    assert_eq!(reg.scope, Scope::Global);
}

#[test]
fn subscriber_with_scope_datacenter() {
    let reg = SubscriberRegistration::new("svc").with_scope(Scope::DataCenter);
    assert_eq!(reg.scope, Scope::DataCenter);
}

#[test]
fn subscriber_with_app_name_sets_value() {
    let reg = SubscriberRegistration::new("svc").with_app_name("subscriber-app");
    assert_eq!(reg.app_name.as_deref(), Some("subscriber-app"));
}

#[test]
fn subscriber_builder_chain() {
    let reg = SubscriberRegistration::new("com.example.Service")
        .with_group("grp")
        .with_instance_id("inst")
        .with_scope(Scope::Global)
        .with_app_name("app");
    assert_eq!(reg.data_id, "com.example.Service");
    assert_eq!(reg.group, "grp");
    assert_eq!(reg.instance_id, "inst");
    assert_eq!(reg.scope, Scope::Global);
    assert_eq!(reg.app_name.as_deref(), Some("app"));
}

#[test]
fn subscriber_data_info_id_default_format() {
    let reg = SubscriberRegistration::new("com.example.Svc");
    assert_eq!(
        reg.data_info_id(),
        "com.example.Svc#DEFAULT_INSTANCE_ID#DEFAULT_GROUP"
    );
}

#[test]
fn subscriber_data_info_id_with_custom_values() {
    let reg = SubscriberRegistration::new("svc")
        .with_instance_id("inst-a")
        .with_group("grp-b");
    assert_eq!(reg.data_info_id(), "svc#inst-a#grp-b");
}

#[test]
fn subscriber_new_accepts_string() {
    let reg = SubscriberRegistration::new(String::from("owned-svc"));
    assert_eq!(reg.data_id, "owned-svc");
}

#[test]
fn subscriber_clone_is_independent() {
    let reg1 = SubscriberRegistration::new("svc").with_scope(Scope::Zone);
    let mut reg2 = reg1.clone();
    reg2.scope = Scope::Global;
    assert_eq!(reg1.scope, Scope::Zone);
    assert_eq!(reg2.scope, Scope::Global);
}

#[test]
fn subscriber_debug_impl() {
    let reg = SubscriberRegistration::new("svc");
    let dbg = format!("{:?}", reg);
    assert!(dbg.contains("SubscriberRegistration"));
    assert!(dbg.contains("svc"));
}

// ---------------------------------------------------------------------------
// data_info_id format consistency between Publisher and Subscriber
// ---------------------------------------------------------------------------

#[test]
fn data_info_id_format_consistent_between_publisher_and_subscriber() {
    let pub_reg = PublisherRegistration::new("svc")
        .with_instance_id("inst")
        .with_group("grp");
    let sub_reg = SubscriberRegistration::new("svc")
        .with_instance_id("inst")
        .with_group("grp");
    assert_eq!(pub_reg.data_info_id(), sub_reg.data_info_id());
    assert_eq!(pub_reg.data_info_id(), "svc#inst#grp");
}

#[test]
fn data_info_id_with_special_characters() {
    let reg = PublisherRegistration::new("com.example:service/v2")
        .with_instance_id("inst-123")
        .with_group("group.a");
    assert_eq!(
        reg.data_info_id(),
        "com.example:service/v2#inst-123#group.a"
    );
}
