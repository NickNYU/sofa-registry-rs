use async_trait::async_trait;
use sofa_registry_core::error::{RegistryError, Result};
use sqlx::SqlitePool;

use crate::traits::InterfaceAppsRepository;

pub struct SqliteInterfaceAppsRepo {
    pool: SqlitePool,
}

impl SqliteInterfaceAppsRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl InterfaceAppsRepository for SqliteInterfaceAppsRepo {
    async fn get_app_names(&self, data_center: &str, interface_name: &str) -> Result<Vec<String>> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT app_name FROM interface_apps_index \
             WHERE data_center = ? AND interface_name = ?",
        )
        .bind(data_center)
        .bind(interface_name)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RegistryError::Database(e.to_string()))?;

        Ok(rows.into_iter().map(|(name,)| name).collect())
    }

    async fn register(
        &self,
        data_center: &str,
        app_name: &str,
        interface_name: &str,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO interface_apps_index (data_center, app_name, interface_name, gmt_create, gmt_modified) \
             VALUES (?, ?, ?, datetime('now'), datetime('now')) \
             ON CONFLICT(data_center, app_name, interface_name) DO UPDATE SET \
             gmt_modified = datetime('now')",
        )
        .bind(data_center)
        .bind(app_name)
        .bind(interface_name)
        .execute(&self.pool)
        .await
        .map_err(|e| RegistryError::Database(e.to_string()))?;

        Ok(())
    }
}
