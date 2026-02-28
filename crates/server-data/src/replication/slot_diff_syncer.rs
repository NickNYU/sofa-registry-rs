use sofa_registry_store::traits::DatumStorage;
use std::collections::HashMap;
use std::sync::Arc;

/// Compact digest of a single datum (data_info_id) for diff comparison.
#[derive(Debug, Clone)]
pub struct DatumDigest {
    pub data_info_id: String,
    pub version: i64,
    pub publisher_count: u32,
}

/// Result of comparing local and remote digests.
#[derive(Debug, Clone, Default)]
pub struct DiffResult {
    /// data_info_ids whose remote version is newer or whose publisher count differs.
    pub updated: Vec<String>,
    /// data_info_ids that exist locally but not on the remote (should be removed).
    pub removed: Vec<String>,
}

/// Handles diff-based replication between slot leader and followers.
pub struct SlotDiffSyncer {
    storage: Arc<dyn DatumStorage>,
    slot_num: u32,
}

impl SlotDiffSyncer {
    pub fn new(storage: Arc<dyn DatumStorage>, slot_num: u32) -> Self {
        Self { storage, slot_num }
    }

    fn slot_of(&self, data_info_id: &str) -> u32 {
        let hash = crc32c::crc32c(data_info_id.as_bytes());
        hash % self.slot_num
    }

    /// Compute a digest of all datums for a specific slot in the given data center.
    pub fn compute_slot_digest(&self, data_center: &str, slot_id: u32) -> Vec<DatumDigest> {
        let all_versions = self.storage.get_all_versions(data_center);
        let mut digests = Vec::new();

        for (data_info_id, version) in all_versions {
            if self.slot_of(&data_info_id) != slot_id {
                continue;
            }
            let publishers = self.storage.get_publishers(data_center, &data_info_id);
            digests.push(DatumDigest {
                data_info_id,
                version: version.value,
                publisher_count: publishers.len() as u32,
            });
        }

        digests
    }

    /// Compare local digests against remote digests to find what needs syncing.
    ///
    /// - `updated`: data_info_ids where the remote is newer or has a different
    ///   publisher count (follower should fetch from leader).
    /// - `removed`: data_info_ids that exist locally but not on the remote
    ///   (follower should delete them).
    pub fn diff_digests(local: &[DatumDigest], remote: &[DatumDigest]) -> DiffResult {
        let remote_map: HashMap<&str, &DatumDigest> =
            remote.iter().map(|d| (d.data_info_id.as_str(), d)).collect();

        let local_map: HashMap<&str, &DatumDigest> =
            local.iter().map(|d| (d.data_info_id.as_str(), d)).collect();

        let mut result = DiffResult::default();

        // Find updated: remote has newer version or different count.
        for (id, remote_digest) in &remote_map {
            match local_map.get(id) {
                Some(local_digest) => {
                    if remote_digest.version > local_digest.version
                        || remote_digest.publisher_count != local_digest.publisher_count
                    {
                        result.updated.push(id.to_string());
                    }
                }
                None => {
                    // Exists on remote but not locally.
                    result.updated.push(id.to_string());
                }
            }
        }

        // Find removed: exists locally but not on remote.
        for id in local_map.keys() {
            if !remote_map.contains_key(id) {
                result.removed.push(id.to_string());
            }
        }

        result
    }
}
