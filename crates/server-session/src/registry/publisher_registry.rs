use dashmap::DashMap;
use sofa_registry_core::model::Publisher;
use std::collections::HashMap;

/// Tracks all publishers registered via this session server.
///
/// Two indexes are maintained:
/// - `by_connection`:  connect_id (String) -> (data_info_id -> Publisher)
/// - `by_data_info_id`: data_info_id -> Vec<Publisher>
pub struct PublisherRegistry {
    by_connection: DashMap<String, HashMap<String, Publisher>>,
    by_data_info_id: DashMap<String, Vec<Publisher>>,
}

impl PublisherRegistry {
    pub fn new() -> Self {
        Self {
            by_connection: DashMap::new(),
            by_data_info_id: DashMap::new(),
        }
    }

    /// Register a publisher. Returns `true` if this is a new registration,
    /// `false` if it replaces an existing publisher with the same regist_id under
    /// the same data_info_id.
    pub fn register(&self, publisher: Publisher) -> bool {
        let connect_id = publisher.source_address.to_string();
        let data_info_id = publisher.data_info_id.clone();
        let regist_id = publisher.regist_id.clone();

        // Update by_connection index
        let is_new = {
            let mut conn_entry = self.by_connection.entry(connect_id).or_default();
            // Key by data_info_id so that one connection can publish to multiple data_info_ids
            // but only one publisher per data_info_id per connection is kept (keyed by regist_id
            // below we actually key by data_info_id for the connection-level map).
            // Actually let's key by regist_id within the connection for correctness.
            conn_entry
                .insert(regist_id.clone(), publisher.clone())
                .is_none()
        };

        // Update by_data_info_id index
        {
            let mut list = self.by_data_info_id.entry(data_info_id).or_default();
            // Remove any existing entry with same regist_id
            list.retain(|p| p.regist_id != regist_id);
            list.push(publisher);
        }

        is_new
    }

    /// Unregister a specific publisher by data_info_id and regist_id.
    /// Returns the removed publisher if found.
    pub fn unregister(&self, data_info_id: &str, regist_id: &str) -> Option<Publisher> {
        // Remove from by_data_info_id and extract the publisher data we need
        let removed = {
            let mut entry = self.by_data_info_id.get_mut(data_info_id)?;
            let idx = entry.iter().position(|p| p.regist_id == regist_id)?;
            let publisher = entry.remove(idx);
            Some(publisher)
        };

        if let Some(ref publisher) = removed {
            // Immediately update by_connection while we know the connect_id
            let connect_id = publisher.source_address.to_string();
            if let Some(mut conn_map) = self.by_connection.get_mut(&connect_id) {
                conn_map.remove(regist_id);
            }
            // Atomically remove the connection entry only if it is empty,
            // avoiding the TOCTOU race of check-then-remove.
            self.by_connection
                .remove_if(&connect_id, |_, v| v.is_empty());
        }

        // Atomically remove the data_info_id entry only if it is empty
        self.by_data_info_id
            .remove_if(data_info_id, |_, v| v.is_empty());

        removed
    }

    /// Get all publishers for a given data_info_id.
    pub fn get_by_data_info_id(&self, data_info_id: &str) -> Vec<Publisher> {
        self.by_data_info_id
            .get(data_info_id)
            .map(|v| v.clone())
            .unwrap_or_default()
    }

    /// Get all publishers registered from a specific connection.
    pub fn get_by_connect_id(&self, connect_id: &str) -> Vec<Publisher> {
        self.by_connection
            .get(connect_id)
            .map(|m| m.values().cloned().collect())
            .unwrap_or_default()
    }

    /// Remove all publishers associated with a given connection.
    /// Returns all removed publishers.
    pub fn remove_by_connect_id(&self, connect_id: &str) -> Vec<Publisher> {
        let removed = match self.by_connection.remove(connect_id) {
            Some((_, map)) => map.into_values().collect::<Vec<_>>(),
            None => return Vec::new(),
        };

        // Remove from by_data_info_id index
        for publisher in &removed {
            if let Some(mut list) = self.by_data_info_id.get_mut(&publisher.data_info_id) {
                list.retain(|p| p.regist_id != publisher.regist_id);
            }
            // Atomically remove the data_info_id entry only if it is empty,
            // avoiding the TOCTOU race of check-then-remove.
            self.by_data_info_id
                .remove_if(&publisher.data_info_id, |_, v| v.is_empty());
        }

        removed
    }

    /// Total number of publisher registrations.
    pub fn count(&self) -> usize {
        self.by_data_info_id.iter().map(|e| e.value().len()).sum()
    }

    /// Number of distinct data_info_ids with at least one publisher.
    pub fn data_info_id_count(&self) -> usize {
        self.by_data_info_id.len()
    }
}

impl Default for PublisherRegistry {
    fn default() -> Self {
        Self::new()
    }
}
