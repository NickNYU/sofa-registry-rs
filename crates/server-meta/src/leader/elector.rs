use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use parking_lot::RwLock;
use tokio::time::{Duration, interval};
use tokio_util::sync::CancellationToken;
use tracing::{info, warn, error, debug};
use sofa_registry_core::error::Result;
use sofa_registry_core::constants::defaults;
use sofa_registry_server_shared::metrics as srv_metrics;
use sofa_registry_store::traits::{
    leader_elector::{LeaderInfo, ElectorRole, LeaderAware, LeaderElector},
    distribute_lock::DistributeLockRepository,
};

/// JDBC-backed leader elector for Meta server.
/// Translated from Java's MetaJdbcLeaderElector / AbstractLeaderElector.
///
/// Election algorithm:
/// 1. All nodes start as FOLLOWER
/// 2. Every election_interval, each node either competes (FOLLOWER) or queries (OBSERVER)
/// 3. Competition: try to acquire distribute_lock row. First to insert/update wins.
/// 4. If a node wins, it becomes LEADER and heartbeats the lock to extend it.
/// 5. If the lock expires, followers re-compete.
pub struct MetaLeaderElector {
    lock_repo: Arc<dyn DistributeLockRepository>,
    my_address: String,
    data_center: String,
    lock_duration_ms: i64,
    election_interval_ms: u64,
    
    leader_info: RwLock<LeaderInfo>,
    role: RwLock<ElectorRole>,
    awares: RwLock<Vec<Arc<dyn LeaderAware>>>,
    was_leader: AtomicBool,
}

impl MetaLeaderElector {
    pub fn new(
        lock_repo: Arc<dyn DistributeLockRepository>,
        my_address: String,
        data_center: String,
        lock_duration_ms: i64,
        election_interval_ms: u64,
    ) -> Self {
        Self {
            lock_repo,
            my_address,
            data_center,
            lock_duration_ms,
            election_interval_ms,
            leader_info: RwLock::new(LeaderInfo::empty()),
            role: RwLock::new(ElectorRole::Follower),
            awares: RwLock::new(Vec::new()),
            was_leader: AtomicBool::new(false),
        }
    }

    /// Main election loop - runs until cancelled.
    pub async fn run_election_loop(&self, cancel: CancellationToken) {
        info!("Starting election loop for {} (interval={}ms)", self.my_address, self.election_interval_ms);
        let mut ticker = interval(Duration::from_millis(self.election_interval_ms));
        
        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    info!("Election loop cancelled");
                    break;
                }
                _ = ticker.tick() => {
                    if let Err(e) = self.do_election_tick().await {
                        error!("Election tick failed: {}", e);
                    }
                }
            }
        }
    }

    async fn do_election_tick(&self) -> Result<()> {
        metrics::counter!(srv_metrics::META_ELECTIONS_TOTAL).increment(1);
        let role = *self.role.read();
        match role {
            ElectorRole::Leader => {
                // Heartbeat to extend lock
                self.do_leader_heartbeat().await
            }
            ElectorRole::Follower => {
                // Try to compete for leadership
                self.do_compete().await
            }
            ElectorRole::Observer => {
                // Just query who is leader
                self.do_query().await
            }
        }
    }

    async fn do_compete(&self) -> Result<()> {
        debug!("Competing for leadership: {}", self.my_address);
        
        match self.lock_repo.compete_lock(
            defaults::META_LEADER_LOCK_NAME,
            &self.data_center,
            &self.my_address,
            self.lock_duration_ms,
        ).await {
            Ok(Some(lock)) => {
                let is_me = lock.owner == self.my_address;
                let leader_info = LeaderInfo {
                    epoch: lock.term,
                    leader: Some(lock.owner.clone()),
                    expire_timestamp: lock.expire_timestamp(),
                };
                
                self.update_leader_info(leader_info, is_me);
                
                if is_me {
                    info!("I am now the leader: {} (term={})", self.my_address, lock.term);
                } else {
                    debug!("Leader is: {} (term={})", lock.owner, lock.term);
                }
            }
            Ok(None) => {
                debug!("No leader elected yet");
            }
            Err(e) => {
                warn!("Failed to compete for leadership: {}", e);
            }
        }
        
        Ok(())
    }

    async fn do_leader_heartbeat(&self) -> Result<()> {
        match self.lock_repo.owner_heartbeat(
            defaults::META_LEADER_LOCK_NAME,
            &self.data_center,
            &self.my_address,
            self.lock_duration_ms,
        ).await {
            Ok(true) => {
                debug!("Leader heartbeat successful");
            }
            Ok(false) => {
                warn!("Lost leadership - heartbeat failed, reverting to follower");
                self.change_to_follower();
            }
            Err(e) => {
                warn!("Leader heartbeat error: {}, reverting to follower", e);
                self.change_to_follower();
            }
        }
        Ok(())
    }

    async fn do_query(&self) -> Result<()> {
        match self.lock_repo.query_lock(
            defaults::META_LEADER_LOCK_NAME,
            &self.data_center,
        ).await {
            Ok(Some(lock)) if !lock.is_expired() => {
                let expire_timestamp = lock.expire_timestamp();
                let leader_info = LeaderInfo {
                    epoch: lock.term,
                    leader: Some(lock.owner),
                    expire_timestamp,
                };
                *self.leader_info.write() = leader_info;
            }
            _ => {
                *self.leader_info.write() = LeaderInfo::empty();
            }
        }
        Ok(())
    }

    fn update_leader_info(&self, info: LeaderInfo, is_me: bool) {
        let was_leader = self.was_leader.load(Ordering::Relaxed);
        
        *self.leader_info.write() = info;
        
        if is_me && !was_leader {
            // Became leader
            *self.role.write() = ElectorRole::Leader;
            self.was_leader.store(true, Ordering::Relaxed);
            self.notify_leader();
        } else if !is_me && was_leader {
            // Lost leadership
            *self.role.write() = ElectorRole::Follower;
            self.was_leader.store(false, Ordering::Relaxed);
            self.notify_follower();
        } else if is_me {
            *self.role.write() = ElectorRole::Leader;
        }
    }

    fn notify_leader(&self) {
        for aware in self.awares.read().iter() {
            aware.on_become_leader();
        }
    }

    fn notify_follower(&self) {
        for aware in self.awares.read().iter() {
            aware.on_lose_leadership();
        }
    }
}

#[async_trait::async_trait]
impl LeaderElector for MetaLeaderElector {
    async fn elect(&self) -> Result<LeaderInfo> {
        self.do_compete().await?;
        Ok(self.get_leader_info())
    }

    async fn query_leader(&self) -> Result<LeaderInfo> {
        self.do_query().await?;
        Ok(self.get_leader_info())
    }

    fn am_i_leader(&self) -> bool {
        *self.role.read() == ElectorRole::Leader
    }

    fn get_leader_info(&self) -> LeaderInfo {
        self.leader_info.read().clone()
    }

    fn get_role(&self) -> ElectorRole {
        *self.role.read()
    }

    fn myself(&self) -> &str {
        &self.my_address
    }

    fn change_to_follower(&self) {
        let was_leader = *self.role.read() == ElectorRole::Leader;
        *self.role.write() = ElectorRole::Follower;
        if was_leader {
            self.was_leader.store(false, Ordering::Relaxed);
            self.notify_follower();
        }
    }

    fn change_to_observer(&self) {
        let was_leader = *self.role.read() == ElectorRole::Leader;
        *self.role.write() = ElectorRole::Observer;
        if was_leader {
            self.was_leader.store(false, Ordering::Relaxed);
            self.notify_follower();
        }
    }

    fn register_leader_aware(&self, aware: Arc<dyn LeaderAware>) {
        self.awares.write().push(aware);
    }
}
