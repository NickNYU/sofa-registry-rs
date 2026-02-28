/// Utility for dataInfoId format: "dataId#instanceId#group"
pub struct DataInfo;

impl DataInfo {
    pub fn to_data_info_id(data_id: &str, instance_id: &str, group: &str) -> String {
        format!("{}#{}#{}", data_id, instance_id, group)
    }

    pub fn parse(data_info_id: &str) -> Option<(String, String, String)> {
        let parts: Vec<&str> = data_info_id.splitn(3, '#').collect();
        if parts.len() == 3 {
            Some((
                parts[0].to_string(),
                parts[1].to_string(),
                parts[2].to_string(),
            ))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_data_info_id() {
        let result = DataInfo::to_data_info_id("com.example.Service", "default", "DEFAULT_GROUP");
        assert_eq!(result, "com.example.Service#default#DEFAULT_GROUP");
    }

    #[test]
    fn test_parse_valid() {
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
    fn test_parse_invalid() {
        assert_eq!(DataInfo::parse("no-hash-here"), None);
        assert_eq!(DataInfo::parse("one#two"), None);
    }

    #[test]
    fn test_parse_with_hash_in_group() {
        // group can contain '#' since we use splitn(3, ...)
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
}
