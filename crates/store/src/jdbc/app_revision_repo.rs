use async_trait::async_trait;
use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use sofa_registry_core::error::{RegistryError, Result};
use sqlx::SqlitePool;
use std::collections::HashMap;

use crate::traits::{AppRevision, AppRevisionRepository};

pub struct SqliteAppRevisionRepo {
    pool: SqlitePool,
}

impl SqliteAppRevisionRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    fn parse_datetime(s: &str) -> DateTime<Utc> {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
            .map(|dt| Utc.from_utc_datetime(&dt))
            .unwrap_or_else(|_| Utc::now())
    }

    fn row_to_revision(
        row: (
            String,
            String,
            String,
            Option<String>,
            Option<String>,
            i32,
            String,
            String,
        ),
    ) -> AppRevision {
        let base_params: HashMap<String, String> = row
            .3
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();
        let service_params: HashMap<String, HashMap<String, String>> = row
            .4
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();

        AppRevision {
            data_center: row.0,
            revision: row.1,
            app_name: row.2,
            base_params,
            service_params,
            deleted: row.5 != 0,
            gmt_create: Self::parse_datetime(&row.6),
            gmt_modified: Self::parse_datetime(&row.7),
        }
    }
}

#[async_trait]
impl AppRevisionRepository for SqliteAppRevisionRepo {
    async fn register(&self, revision: AppRevision) -> Result<()> {
        let base_params_json = serde_json::to_string(&revision.base_params).unwrap_or_default();
        let service_params_json =
            serde_json::to_string(&revision.service_params).unwrap_or_default();
        let deleted_int: i32 = if revision.deleted { 1 } else { 0 };

        sqlx::query(
            "INSERT INTO app_revision (data_center, revision, app_name, base_params, service_params, deleted, gmt_create, gmt_modified) \
             VALUES (?, ?, ?, ?, ?, ?, datetime('now'), datetime('now')) \
             ON CONFLICT(data_center, revision) DO UPDATE SET \
             app_name = excluded.app_name, base_params = excluded.base_params, \
             service_params = excluded.service_params, deleted = excluded.deleted, \
             gmt_modified = datetime('now')",
        )
        .bind(&revision.data_center)
        .bind(&revision.revision)
        .bind(&revision.app_name)
        .bind(&base_params_json)
        .bind(&service_params_json)
        .bind(deleted_int)
        .execute(&self.pool)
        .await
        .map_err(|e| RegistryError::Database(e.to_string()))?;

        Ok(())
    }

    async fn query_revision(&self, revision: &str) -> Result<Option<AppRevision>> {
        let row: Option<(
            String,
            String,
            String,
            Option<String>,
            Option<String>,
            i32,
            String,
            String,
        )> = sqlx::query_as(
            "SELECT data_center, revision, app_name, base_params, service_params, deleted, \
                        CAST(gmt_create AS TEXT), CAST(gmt_modified AS TEXT) \
                 FROM app_revision WHERE revision = ?",
        )
        .bind(revision)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RegistryError::Database(e.to_string()))?;

        Ok(row.map(Self::row_to_revision))
    }

    async fn heartbeat(&self, revision: &str) -> Result<bool> {
        let result = sqlx::query(
            "UPDATE app_revision SET gmt_modified = datetime('now') WHERE revision = ? AND deleted = 0",
        )
        .bind(revision)
        .execute(&self.pool)
        .await
        .map_err(|e| RegistryError::Database(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }

    async fn get_expired(&self, before: DateTime<Utc>, limit: i32) -> Result<Vec<AppRevision>> {
        let before_str = before.format("%Y-%m-%d %H:%M:%S").to_string();
        let rows: Vec<(
            String,
            String,
            String,
            Option<String>,
            Option<String>,
            i32,
            String,
            String,
        )> = sqlx::query_as(
            "SELECT data_center, revision, app_name, base_params, service_params, deleted, \
                        CAST(gmt_create AS TEXT), CAST(gmt_modified AS TEXT) \
                 FROM app_revision WHERE gmt_modified < ? AND deleted = 0 LIMIT ?",
        )
        .bind(&before_str)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RegistryError::Database(e.to_string()))?;

        Ok(rows.into_iter().map(Self::row_to_revision).collect())
    }

    async fn clean_deleted(&self, before: DateTime<Utc>, limit: i32) -> Result<i32> {
        let before_str = before.format("%Y-%m-%d %H:%M:%S").to_string();
        // SQLite doesn't support DELETE ... LIMIT directly in all versions,
        // so we use a subquery approach.
        let result = sqlx::query(
            "DELETE FROM app_revision WHERE id IN (\
                SELECT id FROM app_revision WHERE deleted = 1 AND gmt_modified < ? LIMIT ?\
             )",
        )
        .bind(&before_str)
        .bind(limit)
        .execute(&self.pool)
        .await
        .map_err(|e| RegistryError::Database(e.to_string()))?;

        Ok(result.rows_affected() as i32)
    }
}
