use async_trait::async_trait;
use chrono::{NaiveDateTime, TimeZone, Utc};
use sofa_registry_core::error::{RegistryError, Result};
use sqlx::SqlitePool;

use crate::traits::{ClientManagerAddress, ClientManagerAddressRepository};

pub struct SqliteClientManagerRepo {
    pool: SqlitePool,
}

impl SqliteClientManagerRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    fn parse_datetime(s: &str) -> chrono::DateTime<Utc> {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
            .map(|dt| Utc.from_utc_datetime(&dt))
            .unwrap_or_else(|_| Utc::now())
    }
}

#[async_trait]
impl ClientManagerAddressRepository for SqliteClientManagerRepo {
    async fn get_client_off_addresses(
        &self,
        data_center: &str,
    ) -> Result<Vec<ClientManagerAddress>> {
        let rows: Vec<(String, String, String, String)> = sqlx::query_as(
            "SELECT data_center, address, operation, CAST(gmt_create AS TEXT) \
             FROM client_manager_address \
             WHERE data_center = ? AND operation = 'CLIENT_OFF'",
        )
        .bind(data_center)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RegistryError::Database(e.to_string()))?;

        Ok(rows
            .into_iter()
            .map(|(dc, addr, op, created)| ClientManagerAddress {
                data_center: dc,
                address: addr,
                operation: op,
                gmt_create: Self::parse_datetime(&created),
            })
            .collect())
    }

    async fn client_off(&self, data_center: &str, address: &str) -> Result<bool> {
        let result = sqlx::query(
            "INSERT INTO client_manager_address (data_center, address, operation, gmt_create, gmt_modified) \
             VALUES (?, ?, 'CLIENT_OFF', datetime('now'), datetime('now')) \
             ON CONFLICT(data_center, address) DO UPDATE SET \
             operation = 'CLIENT_OFF', gmt_modified = datetime('now')",
        )
        .bind(data_center)
        .bind(address)
        .execute(&self.pool)
        .await
        .map_err(|e| RegistryError::Database(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }

    async fn client_on(&self, data_center: &str, address: &str) -> Result<bool> {
        let result = sqlx::query(
            "DELETE FROM client_manager_address \
             WHERE data_center = ? AND address = ?",
        )
        .bind(data_center)
        .bind(address)
        .execute(&self.pool)
        .await
        .map_err(|e| RegistryError::Database(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }
}
