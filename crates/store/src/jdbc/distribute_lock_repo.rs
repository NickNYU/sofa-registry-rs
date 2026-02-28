use async_trait::async_trait;
use chrono::{NaiveDateTime, TimeZone, Utc};
use sofa_registry_core::error::{RegistryError, Result};
use sqlx::SqlitePool;

use crate::traits::{DistributeLock, DistributeLockRepository};

pub struct SqliteDistributeLockRepo {
    pool: SqlitePool,
}

impl SqliteDistributeLockRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    fn parse_datetime(s: &str) -> chrono::DateTime<Utc> {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
            .map(|dt| Utc.from_utc_datetime(&dt))
            .unwrap_or_else(|e| {
                tracing::warn!(
                    input = s,
                    error = %e,
                    "failed to parse datetime, falling back to Utc::now()"
                );
                Utc::now()
            })
    }
}

#[async_trait]
impl DistributeLockRepository for SqliteDistributeLockRepo {
    async fn compete_lock(
        &self,
        lock_name: &str,
        data_center: &str,
        owner: &str,
        duration_ms: i64,
    ) -> Result<Option<DistributeLock>> {
        // Try to insert a new lock row.
        let insert_result = sqlx::query(
            "INSERT INTO distribute_lock (data_center, lock_name, owner, duration, term, term_duration, gmt_create, gmt_modified) \
             VALUES (?, ?, ?, ?, 1, ?, datetime('now'), datetime('now'))"
        )
            .bind(data_center)
            .bind(lock_name)
            .bind(owner)
            .bind(duration_ms)
            .bind(duration_ms)
            .execute(&self.pool)
            .await;

        match insert_result {
            Ok(_) => {
                // Successfully inserted, we hold the lock.
                return self.query_lock(lock_name, data_center).await;
            }
            Err(_) => {
                // Row exists. Try to take over if the lock is expired or we are the owner.
                let now_ms = Utc::now().timestamp_millis();
                // Query the existing lock to check expiry.
                let existing = self.query_lock(lock_name, data_center).await?;
                if let Some(ref lock) = existing {
                    if lock.owner == owner || lock.is_expired() {
                        let update_result = sqlx::query(
                            "UPDATE distribute_lock \
                             SET owner = ?, duration = ?, term = term + 1, term_duration = ?, gmt_modified = datetime('now') \
                             WHERE data_center = ? AND lock_name = ? AND (owner = ? OR (CAST(strftime('%s', gmt_modified) AS INTEGER) * 1000 + duration) < ?)"
                        )
                            .bind(owner)
                            .bind(duration_ms)
                            .bind(duration_ms)
                            .bind(data_center)
                            .bind(lock_name)
                            .bind(&lock.owner)
                            .bind(now_ms)
                            .execute(&self.pool)
                            .await
                            .map_err(|e| RegistryError::Database(e.to_string()))?;

                        if update_result.rows_affected() > 0 {
                            return self.query_lock(lock_name, data_center).await;
                        }
                    }
                }
                Ok(None)
            }
        }
    }

    async fn query_lock(
        &self,
        lock_name: &str,
        data_center: &str,
    ) -> Result<Option<DistributeLock>> {
        let row: Option<(String, String, String, i64, i64, i64, String, String)> = sqlx::query_as(
            "SELECT data_center, lock_name, owner, duration, term, term_duration, \
                    CAST(gmt_create AS TEXT), CAST(gmt_modified AS TEXT) \
             FROM distribute_lock WHERE data_center = ? AND lock_name = ?",
        )
        .bind(data_center)
        .bind(lock_name)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RegistryError::Database(e.to_string()))?;

        Ok(row.map(
            |(dc, ln, ow, dur, term, term_dur, created, modified)| DistributeLock {
                data_center: dc,
                lock_name: ln,
                owner: ow,
                duration: dur,
                term,
                term_duration: term_dur,
                gmt_create: Self::parse_datetime(&created),
                gmt_modified: Self::parse_datetime(&modified),
            },
        ))
    }

    async fn owner_heartbeat(
        &self,
        lock_name: &str,
        data_center: &str,
        owner: &str,
        duration_ms: i64,
    ) -> Result<bool> {
        let result = sqlx::query(
            "UPDATE distribute_lock SET duration = ?, term_duration = term_duration + ?, gmt_modified = datetime('now') \
             WHERE data_center = ? AND lock_name = ? AND owner = ?",
        )
        .bind(duration_ms)
        .bind(duration_ms)
        .bind(data_center)
        .bind(lock_name)
        .bind(owner)
        .execute(&self.pool)
        .await
        .map_err(|e| RegistryError::Database(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }
}
