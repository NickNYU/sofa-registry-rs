use async_trait::async_trait;
use sofa_registry_core::error::{RegistryError, Result};
use sqlx::SqlitePool;

use crate::traits::{PersistenceData, ProvideDataRepository};

pub struct SqliteProvideDataRepo {
    pool: SqlitePool,
}

impl SqliteProvideDataRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ProvideDataRepository for SqliteProvideDataRepo {
    async fn put(&self, data: PersistenceData) -> Result<bool> {
        let result = sqlx::query(
            "INSERT INTO provide_data (data_center, data_key, data_value, version, gmt_create, gmt_modified) \
             VALUES (?, ?, ?, ?, datetime('now'), datetime('now')) \
             ON CONFLICT(data_center, data_key) DO UPDATE SET \
             data_value = excluded.data_value, version = excluded.version, \
             gmt_modified = datetime('now')",
        )
        .bind(&data.data_center)
        .bind(&data.data_key)
        .bind(&data.data_value)
        .bind(data.version)
        .execute(&self.pool)
        .await
        .map_err(|e| RegistryError::Database(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }

    async fn get(&self, data_center: &str, data_key: &str) -> Result<Option<PersistenceData>> {
        let row: Option<(String, String, Option<String>, i64)> = sqlx::query_as(
            "SELECT data_center, data_key, data_value, version \
             FROM provide_data WHERE data_center = ? AND data_key = ?",
        )
        .bind(data_center)
        .bind(data_key)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RegistryError::Database(e.to_string()))?;

        Ok(row.map(|(dc, dk, dv, ver)| PersistenceData {
            data_center: dc,
            data_key: dk,
            data_value: dv.unwrap_or_default(),
            version: ver,
        }))
    }

    async fn remove(&self, data_center: &str, data_key: &str) -> Result<bool> {
        let result = sqlx::query(
            "DELETE FROM provide_data WHERE data_center = ? AND data_key = ?",
        )
        .bind(data_center)
        .bind(data_key)
        .execute(&self.pool)
        .await
        .map_err(|e| RegistryError::Database(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }

    async fn get_all(&self, data_center: &str) -> Result<Vec<PersistenceData>> {
        let rows: Vec<(String, String, Option<String>, i64)> = sqlx::query_as(
            "SELECT data_center, data_key, data_value, version \
             FROM provide_data WHERE data_center = ?",
        )
        .bind(data_center)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RegistryError::Database(e.to_string()))?;

        Ok(rows
            .into_iter()
            .map(|(dc, dk, dv, ver)| PersistenceData {
                data_center: dc,
                data_key: dk,
                data_value: dv.unwrap_or_default(),
                version: ver,
            })
            .collect())
    }
}
