use parking_lot::RwLock;
use std::collections::HashMap;
use tonic::transport::{Channel, Endpoint};
use tracing::{debug, info, warn};

/// Metadata for a cached channel.
struct ChannelEntry {
    channel: Channel,
    last_used: std::time::Instant,
}

/// Manages a pool of gRPC client channels to multiple servers.
/// Supports max connection limits and idle eviction.
pub struct GrpcClientPool {
    channels: RwLock<HashMap<String, ChannelEntry>>,
    connect_timeout: std::time::Duration,
    request_timeout: std::time::Duration,
    max_connections: usize,
    idle_timeout: std::time::Duration,
}

impl GrpcClientPool {
    pub fn new() -> Self {
        Self {
            channels: RwLock::new(HashMap::new()),
            connect_timeout: std::time::Duration::from_secs(5),
            request_timeout: std::time::Duration::from_secs(10),
            max_connections: 256,
            idle_timeout: std::time::Duration::from_secs(300),
        }
    }

    pub fn with_timeouts(connect_timeout_ms: u64, request_timeout_ms: u64) -> Self {
        Self {
            channels: RwLock::new(HashMap::new()),
            connect_timeout: std::time::Duration::from_millis(connect_timeout_ms),
            request_timeout: std::time::Duration::from_millis(request_timeout_ms),
            max_connections: 256,
            idle_timeout: std::time::Duration::from_secs(300),
        }
    }

    /// Get or create a channel to the given address.
    pub async fn get_channel(&self, addr: &str) -> Result<Channel, tonic::transport::Error> {
        // Check cache and update last_used
        if let Some(entry) = self.channels.write().get_mut(addr) {
            entry.last_used = std::time::Instant::now();
            return Ok(entry.channel.clone());
        }

        // Evict idle connections before adding a new one
        self.evict_idle();

        // If still at max, evict the oldest connection
        if self.channels.read().len() >= self.max_connections {
            self.evict_oldest();
        }

        // Create new connection
        let endpoint = Endpoint::from_shared(format!("http://{}", addr))
            .map_err(|e| {
                warn!("Invalid endpoint {}: {}", addr, e);
                e
            })?
            .connect_timeout(self.connect_timeout)
            .timeout(self.request_timeout);

        let channel = endpoint.connect().await?;
        info!("Connected to gRPC server at {}", addr);

        self.channels.write().insert(
            addr.to_string(),
            ChannelEntry {
                channel: channel.clone(),
                last_used: std::time::Instant::now(),
            },
        );
        Ok(channel)
    }

    /// Remove a channel from the pool.
    pub fn remove_channel(&self, addr: &str) {
        self.channels.write().remove(addr);
    }

    /// Get all connected addresses.
    pub fn connected_addresses(&self) -> Vec<String> {
        self.channels.read().keys().cloned().collect()
    }

    /// Number of cached connections.
    pub fn connection_count(&self) -> usize {
        self.channels.read().len()
    }

    /// Clear all channels.
    pub fn clear(&self) {
        self.channels.write().clear();
    }

    /// Evict channels that have been idle beyond the idle timeout.
    fn evict_idle(&self) {
        let cutoff = std::time::Instant::now() - self.idle_timeout;
        let mut channels = self.channels.write();
        let before = channels.len();
        channels.retain(|addr, entry| {
            if entry.last_used < cutoff {
                debug!("Evicting idle gRPC connection to {}", addr);
                false
            } else {
                true
            }
        });
        let evicted = before - channels.len();
        if evicted > 0 {
            info!("Evicted {} idle gRPC connections", evicted);
        }
    }

    /// Evict the least-recently-used connection to make room.
    fn evict_oldest(&self) {
        let mut channels = self.channels.write();
        if let Some(oldest_addr) = channels
            .iter()
            .min_by_key(|(_, entry)| entry.last_used)
            .map(|(addr, _)| addr.clone())
        {
            debug!("Evicting oldest gRPC connection to {}", oldest_addr);
            channels.remove(&oldest_addr);
        }
    }
}

impl Default for GrpcClientPool {
    fn default() -> Self {
        Self::new()
    }
}
