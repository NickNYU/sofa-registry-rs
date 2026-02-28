use sofa_registry_core::model::{Datum, DatumVersion, ProcessId, Publisher};
use std::collections::HashMap;

/// Core storage interface for registry data (in-memory, used by Data Server).
pub trait DatumStorage: Send + Sync {
    fn get(&self, data_center: &str, data_info_id: &str) -> Option<Datum>;

    fn get_version(&self, data_center: &str, data_info_id: &str) -> Option<DatumVersion>;

    fn get_all_versions(&self, data_center: &str) -> HashMap<String, DatumVersion>;

    fn put_publisher(&self, data_center: &str, publisher: Publisher) -> DatumVersion;

    fn remove_publisher(
        &self,
        data_center: &str,
        data_info_id: &str,
        regist_id: &str,
    ) -> Option<DatumVersion>;

    fn remove_publishers_by_session(
        &self,
        data_center: &str,
        session_process_id: &ProcessId,
    ) -> HashMap<String, DatumVersion>;

    fn get_publishers(
        &self,
        data_center: &str,
        data_info_id: &str,
    ) -> HashMap<String, Publisher>;

    fn get_all_data_info_ids(&self, data_center: &str) -> Vec<String>;

    fn clean_slot(&self, data_center: &str, slot_id: u32);

    fn publisher_count(&self, data_center: &str) -> usize;

    fn datum_count(&self, data_center: &str) -> usize;
}
