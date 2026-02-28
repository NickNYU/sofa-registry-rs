use dashmap::DashMap;
use sofa_registry_core::model::Subscriber;
use std::collections::HashMap;

/// Tracks all subscribers registered via this session server.
///
/// Two indexes are maintained:
/// - `by_connection`:   connect_id (String) -> (regist_id -> Subscriber)
/// - `by_data_info_id`: data_info_id -> Vec<Subscriber>
pub struct SubscriberRegistry {
    by_connection: DashMap<String, HashMap<String, Subscriber>>,
    by_data_info_id: DashMap<String, Vec<Subscriber>>,
}

impl SubscriberRegistry {
    pub fn new() -> Self {
        Self {
            by_connection: DashMap::new(),
            by_data_info_id: DashMap::new(),
        }
    }

    /// Register a subscriber. Returns `true` if this is a new registration,
    /// `false` if it replaces an existing subscriber with the same regist_id.
    pub fn register(&self, subscriber: Subscriber) -> bool {
        let connect_id = subscriber.source_address.to_string();
        let data_info_id = subscriber.data_info_id.clone();
        let regist_id = subscriber.regist_id.clone();

        // Update by_connection index
        let is_new = {
            let mut conn_entry = self.by_connection.entry(connect_id).or_default();
            conn_entry
                .insert(regist_id.clone(), subscriber.clone())
                .is_none()
        };

        // Update by_data_info_id index
        {
            let mut list = self.by_data_info_id.entry(data_info_id).or_default();
            list.retain(|s| s.regist_id != regist_id);
            list.push(subscriber);
        }

        is_new
    }

    /// Unregister a specific subscriber by data_info_id and regist_id.
    /// Returns the removed subscriber if found.
    pub fn unregister(&self, data_info_id: &str, regist_id: &str) -> Option<Subscriber> {
        // Remove from by_data_info_id and extract the subscriber data we need
        let removed = {
            let mut entry = self.by_data_info_id.get_mut(data_info_id)?;
            let idx = entry.iter().position(|s| s.regist_id == regist_id)?;
            let subscriber = entry.remove(idx);
            Some(subscriber)
        };

        if let Some(ref subscriber) = removed {
            // Immediately update by_connection while we know the connect_id
            let connect_id = subscriber.source_address.to_string();
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

    /// Get all subscribers for a given data_info_id.
    pub fn get_by_data_info_id(&self, data_info_id: &str) -> Vec<Subscriber> {
        self.by_data_info_id
            .get(data_info_id)
            .map(|v| v.clone())
            .unwrap_or_default()
    }

    /// Remove all subscribers associated with a given connection.
    /// Returns all removed subscribers.
    pub fn remove_by_connect_id(&self, connect_id: &str) -> Vec<Subscriber> {
        let removed = match self.by_connection.remove(connect_id) {
            Some((_, map)) => map.into_values().collect::<Vec<_>>(),
            None => return Vec::new(),
        };

        // Remove from by_data_info_id index
        for subscriber in &removed {
            if let Some(mut list) = self.by_data_info_id.get_mut(&subscriber.data_info_id) {
                list.retain(|s| s.regist_id != subscriber.regist_id);
            }
            // Atomically remove the data_info_id entry only if it is empty,
            // avoiding the TOCTOU race of check-then-remove.
            self.by_data_info_id
                .remove_if(&subscriber.data_info_id, |_, v| v.is_empty());
        }

        removed
    }

    /// Total number of subscriber registrations.
    pub fn count(&self) -> usize {
        self.by_data_info_id.iter().map(|e| e.value().len()).sum()
    }

    /// Number of distinct data_info_ids with at least one subscriber.
    pub fn data_info_id_count(&self) -> usize {
        self.by_data_info_id.len()
    }

    /// Get all data_info_ids that have at least one subscriber.
    pub fn get_all_data_info_ids(&self) -> Vec<String> {
        self.by_data_info_id
            .iter()
            .map(|e| e.key().clone())
            .collect()
    }
}

impl Default for SubscriberRegistry {
    fn default() -> Self {
        Self::new()
    }
}
