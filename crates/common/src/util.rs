/// Format a data_info_id from its components: "dataId#instanceId#group"
pub fn to_data_info_id(data_id: &str, instance_id: &str, group: &str) -> String {
    format!("{}#{}#{}", data_id, instance_id, group)
}

/// Parse a data_info_id into (data_id, instance_id, group)
pub fn parse_data_info_id(data_info_id: &str) -> Option<(&str, &str, &str)> {
    let parts: Vec<&str> = data_info_id.splitn(3, '#').collect();
    if parts.len() == 3 {
        Some((parts[0], parts[1], parts[2]))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_info_id_roundtrip() {
        let id = to_data_info_id("com.example.Service", "DEFAULT_INSTANCE_ID", "DEFAULT_GROUP");
        assert_eq!(id, "com.example.Service#DEFAULT_INSTANCE_ID#DEFAULT_GROUP");
        let (data_id, instance_id, group) = parse_data_info_id(&id).unwrap();
        assert_eq!(data_id, "com.example.Service");
        assert_eq!(instance_id, "DEFAULT_INSTANCE_ID");
        assert_eq!(group, "DEFAULT_GROUP");
    }
}
