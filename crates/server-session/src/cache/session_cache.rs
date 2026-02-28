use dashmap::DashMap;
use sofa_registry_core::model::DatumVersion;

/// Caches datum versions from the data server so the session server can
/// avoid unnecessary fetches when the data has not changed.
pub struct SessionCacheService {
    /// data_info_id -> last known version from data server.
    versions: DashMap<String, DatumVersion>,
}

impl SessionCacheService {
    pub fn new() -> Self {
        Self {
            versions: DashMap::new(),
        }
    }

    /// Get the cached version for a data_info_id, if any.
    pub fn get_version(&self, data_info_id: &str) -> Option<DatumVersion> {
        self.versions.get(data_info_id).map(|v| *v)
    }

    /// Update the cached version for a data_info_id.
    pub fn update_version(&self, data_info_id: &str, version: DatumVersion) {
        self.versions.insert(data_info_id.to_string(), version);
    }

    /// Returns `true` if the remote version is newer than what we have cached,
    /// or if we have no cached version at all.
    pub fn is_stale(&self, data_info_id: &str, remote_version: &DatumVersion) -> bool {
        match self.versions.get(data_info_id) {
            Some(cached) => remote_version.value > cached.value,
            None => true,
        }
    }
}

impl Default for SessionCacheService {
    fn default() -> Self {
        Self::new()
    }
}
