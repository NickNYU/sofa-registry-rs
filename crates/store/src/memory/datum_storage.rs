use crate::traits::DatumStorage;
use dashmap::DashMap;
use sofa_registry_core::model::{Datum, DatumVersion, ProcessId, Publisher};
use std::collections::HashMap;

/// Internal grouping of publishers under a single data_info_id.
struct PublisherGroup {
    publishers: HashMap<String, Publisher>,
    version: DatumVersion,
}

impl PublisherGroup {
    fn new() -> Self {
        Self {
            publishers: HashMap::new(),
            version: DatumVersion::next(),
        }
    }
}

/// In-memory datum storage backed by `DashMap` for concurrent access.
///
/// Layout: data_center -> data_info_id -> PublisherGroup
pub struct LocalDatumStorage {
    store: DashMap<String, DashMap<String, PublisherGroup>>,
    slot_num: u32,
}

impl LocalDatumStorage {
    pub fn new(slot_num: u32) -> Self {
        Self {
            store: DashMap::new(),
            slot_num,
        }
    }

    fn get_or_create_dc_map(
        &self,
        data_center: &str,
    ) -> dashmap::mapref::one::Ref<'_, String, DashMap<String, PublisherGroup>> {
        self.store
            .entry(data_center.to_string())
            .or_default()
            .downgrade()
    }

    fn slot_of(&self, data_info_id: &str) -> u32 {
        let hash = crc32c::crc32c(data_info_id.as_bytes());
        hash % self.slot_num
    }
}

impl DatumStorage for LocalDatumStorage {
    fn get(&self, data_center: &str, data_info_id: &str) -> Option<Datum> {
        let dc_map = self.store.get(data_center)?;
        let group = dc_map.get(data_info_id)?;
        let mut datum = Datum::new_empty(data_info_id, data_center);
        datum.pub_map = group.publishers.clone();
        datum.version = group.version;
        Some(datum)
    }

    fn get_version(&self, data_center: &str, data_info_id: &str) -> Option<DatumVersion> {
        let dc_map = self.store.get(data_center)?;
        let group = dc_map.get(data_info_id)?;
        Some(group.version)
    }

    fn get_all_versions(&self, data_center: &str) -> HashMap<String, DatumVersion> {
        let mut result = HashMap::new();
        if let Some(dc_map) = self.store.get(data_center) {
            for entry in dc_map.iter() {
                result.insert(entry.key().clone(), entry.version);
            }
        }
        result
    }

    fn put_publisher(&self, data_center: &str, publisher: Publisher) -> DatumVersion {
        let dc_map = self.get_or_create_dc_map(data_center);

        let data_info_id = publisher.data_info_id.clone();
        let regist_id = publisher.regist_id.clone();

        let mut group = dc_map
            .entry(data_info_id)
            .or_insert_with(PublisherGroup::new);
        group.publishers.insert(regist_id, publisher);
        group.version = DatumVersion::next();
        group.version
    }

    fn remove_publisher(
        &self,
        data_center: &str,
        data_info_id: &str,
        regist_id: &str,
    ) -> Option<DatumVersion> {
        let dc_map = self.store.get(data_center)?;
        let mut group = dc_map.get_mut(data_info_id)?;
        if group.publishers.remove(regist_id).is_some() {
            group.version = DatumVersion::next();
            Some(group.version)
        } else {
            None
        }
    }

    fn remove_publishers_by_session(
        &self,
        data_center: &str,
        session_process_id: &ProcessId,
    ) -> HashMap<String, DatumVersion> {
        let mut updated = HashMap::new();
        let dc_map = match self.store.get(data_center) {
            Some(m) => m,
            None => return updated,
        };
        for mut entry in dc_map.iter_mut() {
            let key = entry.key().clone();
            let group = entry.value_mut();
            let before = group.publishers.len();
            group
                .publishers
                .retain(|_, pub_item| &pub_item.session_process_id != session_process_id);
            if group.publishers.len() != before {
                group.version = DatumVersion::next();
                updated.insert(key, group.version);
            }
        }
        updated
    }

    fn get_publishers(&self, data_center: &str, data_info_id: &str) -> HashMap<String, Publisher> {
        if let Some(dc_map) = self.store.get(data_center) {
            if let Some(group) = dc_map.get(data_info_id) {
                return group.publishers.clone();
            }
        }
        HashMap::new()
    }

    fn get_all_data_info_ids(&self, data_center: &str) -> Vec<String> {
        match self.store.get(data_center) {
            Some(dc_map) => dc_map.iter().map(|e| e.key().clone()).collect(),
            None => Vec::new(),
        }
    }

    fn clean_slot(&self, data_center: &str, slot_id: u32) {
        if let Some(dc_map) = self.store.get(data_center) {
            dc_map.retain(|data_info_id, _| self.slot_of(data_info_id) != slot_id);
        }
    }

    fn publisher_count(&self, data_center: &str) -> usize {
        match self.store.get(data_center) {
            Some(dc_map) => dc_map.iter().map(|e| e.publishers.len()).sum(),
            None => 0,
        }
    }

    fn datum_count(&self, data_center: &str) -> usize {
        match self.store.get(data_center) {
            Some(dc_map) => dc_map.len(),
            None => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sofa_registry_core::model::{
        ConnectId, ProcessId, PublishSource, PublishType, RegisterVersion,
    };

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

    #[test]
    fn test_put_and_get_publisher() {
        let storage = LocalDatumStorage::new(256);
        let session_pid = ProcessId::new("10.0.0.1", 1000, 1);
        let pub1 = make_publisher("svc#inst#grp", "reg-1", &session_pid);
        storage.put_publisher("dc1", pub1);

        let datum = storage.get("dc1", "svc#inst#grp");
        assert!(datum.is_some());
        let datum = datum.unwrap();
        assert_eq!(datum.pub_map.len(), 1);
        assert!(datum.pub_map.contains_key("reg-1"));
    }

    #[test]
    fn test_remove_publisher() {
        let storage = LocalDatumStorage::new(256);
        let session_pid = ProcessId::new("10.0.0.1", 1000, 1);
        let pub1 = make_publisher("svc#inst#grp", "reg-1", &session_pid);
        storage.put_publisher("dc1", pub1);

        let ver = storage.remove_publisher("dc1", "svc#inst#grp", "reg-1");
        assert!(ver.is_some());

        let pubs = storage.get_publishers("dc1", "svc#inst#grp");
        assert!(pubs.is_empty());
    }

    #[test]
    fn test_remove_publishers_by_session() {
        let storage = LocalDatumStorage::new(256);
        let session_pid = ProcessId::new("10.0.0.1", 1000, 1);
        let other_pid = ProcessId::new("10.0.0.2", 2000, 1);
        let pub1 = make_publisher("svc1#inst#grp", "reg-1", &session_pid);
        let pub2 = make_publisher("svc1#inst#grp", "reg-2", &other_pid);
        storage.put_publisher("dc1", pub1);
        storage.put_publisher("dc1", pub2);

        let updated = storage.remove_publishers_by_session("dc1", &session_pid);
        assert_eq!(updated.len(), 1);

        let pubs = storage.get_publishers("dc1", "svc1#inst#grp");
        assert_eq!(pubs.len(), 1);
        assert!(pubs.contains_key("reg-2"));
    }

    #[test]
    fn test_datum_count_and_publisher_count() {
        let storage = LocalDatumStorage::new(256);
        let session_pid = ProcessId::new("10.0.0.1", 1000, 1);
        let pub1 = make_publisher("svc1#inst#grp", "reg-1", &session_pid);
        let pub2 = make_publisher("svc1#inst#grp", "reg-2", &session_pid);
        let pub3 = make_publisher("svc2#inst#grp", "reg-3", &session_pid);

        storage.put_publisher("dc1", pub1);
        storage.put_publisher("dc1", pub2);
        storage.put_publisher("dc1", pub3);

        assert_eq!(storage.datum_count("dc1"), 2);
        assert_eq!(storage.publisher_count("dc1"), 3);
    }
}
