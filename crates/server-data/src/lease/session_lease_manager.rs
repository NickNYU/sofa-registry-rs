use chrono::Utc;
use dashmap::DashMap;
use std::time::Duration;
use tracing::info;

/// Tracks a single session server's lease.
#[derive(Debug, Clone)]
pub struct SessionLease {
    pub address: String,
    pub process_id: String,
    pub last_renew_timestamp: i64,
    pub expire_timestamp: i64,
}

/// Manages lease tracking for session servers connected to this data server.
pub struct SessionLeaseManager {
    sessions: DashMap<String, SessionLease>,
    lease_duration: Duration,
}

impl SessionLeaseManager {
    pub fn new(lease_duration_secs: u64) -> Self {
        Self {
            sessions: DashMap::new(),
            lease_duration: Duration::from_secs(lease_duration_secs),
        }
    }

    /// Register or renew a session server's lease.
    pub fn renew(&self, address: &str, process_id: &str) {
        let now = Utc::now().timestamp_millis();
        let expire = now + self.lease_duration.as_millis() as i64;

        self.sessions.insert(
            address.to_string(),
            SessionLease {
                address: address.to_string(),
                process_id: process_id.to_string(),
                last_renew_timestamp: now,
                expire_timestamp: expire,
            },
        );
    }

    /// Check if a session server has an active (non-expired) lease.
    pub fn is_active(&self, address: &str) -> bool {
        if let Some(lease) = self.sessions.get(address) {
            let now = Utc::now().timestamp_millis();
            return lease.expire_timestamp > now;
        }
        false
    }

    /// Remove expired sessions and return the list of expired addresses.
    pub fn evict_expired(&self) -> Vec<String> {
        let now = Utc::now().timestamp_millis();
        let mut expired = Vec::new();

        self.sessions.retain(|addr, lease| {
            if lease.expire_timestamp <= now {
                info!("Session lease expired: {}", addr);
                expired.push(addr.clone());
                false
            } else {
                true
            }
        });

        expired
    }

    /// Get all active session addresses.
    pub fn active_sessions(&self) -> Vec<String> {
        let now = Utc::now().timestamp_millis();
        self.sessions
            .iter()
            .filter(|entry| entry.value().expire_timestamp > now)
            .map(|entry| entry.key().clone())
            .collect()
    }

    /// Number of tracked sessions (including potentially expired ones).
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Remove a specific session.
    pub fn remove(&self, address: &str) {
        self.sessions.remove(address);
    }
}
