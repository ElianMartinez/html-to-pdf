use anyhow::{Context, Result};
use chrono::Utc;
use sqlx::{Pool, Sqlite};
use uuid::Uuid;

use crate::models::operation_channel_model::OperationChannelRecord;

#[derive(Clone)]
pub struct NotificationChannelService {
    db_pool: Pool<Sqlite>,
}

impl NotificationChannelService {
    pub fn new(db_pool: Pool<Sqlite>) -> Self {
        NotificationChannelService { db_pool }
    }

    pub async fn create_channel(
        &self,
        operation_id: &str,
        channel: &str,
        initial_status: &str,
    ) -> Result<String> {
        let ch_id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        sqlx::query!(
            r#"
            INSERT INTO operation_channels (
                id, operation_id, channel, status, error_message,
                created_at, updated_at, attempts
            )
            VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?5, 0)
            "#,
            ch_id,
            operation_id,
            channel,
            initial_status,
            now
        )
        .execute(&self.db_pool)
        .await
        .context("Error creando operation_channel")?;

        Ok(ch_id)
    }

    pub async fn update_channel_status(
        &self,
        channel_id: &str,
        status: &str,
        error_message: Option<&str>,
        increment_attempt: bool,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();

        // Si vamos a incrementar attempts, agregamos la asignación
        // con la coma delante, para no dejar comas colgando.
        let attempts_sql = if increment_attempt {
            ", attempts = attempts + 1"
        } else {
            ""
        };

        // Construimos el query
        let sql = format!(
            r#"
            UPDATE operation_channels
            SET
                status = ?1,
                error_message = ?2,
                updated_at = ?3
                {attempts_sql}
            WHERE id = ?4
            "#
        );

        sqlx::query(&sql)
            .bind(status)
            .bind(error_message)
            .bind(now)
            .bind(channel_id)
            .execute(&self.db_pool)
            .await
            .context("Error actualizando operation_channel")?;

        Ok(())
    }

    pub async fn get_channel(&self, channel_id: &str) -> Result<OperationChannelRecord> {
        let row = sqlx::query!(
            r#"
            SELECT id, operation_id, channel, status, error_message,
                   created_at, updated_at, attempts
            FROM operation_channels
            WHERE id = ?1
            "#,
            channel_id
        )
        .fetch_one(&self.db_pool)
        .await
        .context("No se encontró operation_channel")?;

        Ok(OperationChannelRecord {
            id: row.id.expect("id no puede ser NULL"),
            operation_id: row.operation_id,
            channel: row.channel,
            status: row.status,
            error_message: row.error_message,
            created_at: row.created_at.parse()?,
            updated_at: row.updated_at.parse()?,
            attempts: row.attempts as i32,
        })
    }

    pub async fn list_channels_for_operation(
        &self,
        operation_id: &str,
    ) -> Result<Vec<OperationChannelRecord>> {
        let rows = sqlx::query!(
            r#"
            SELECT id, operation_id, channel, status, error_message,
                   created_at, updated_at, attempts
            FROM operation_channels
            WHERE operation_id = ?1
            "#,
            operation_id
        )
        .fetch_all(&self.db_pool)
        .await?;

        let mut result = Vec::new();
        for r in rows {
            let rec = OperationChannelRecord {
                id: r.id.expect("id no puede ser NULL"),
                operation_id: r.operation_id,
                channel: r.channel,
                status: r.status,
                error_message: r.error_message,
                created_at: r.created_at.parse()?,
                updated_at: r.updated_at.parse()?,
                attempts: r.attempts as i32,
            };
            result.push(rec);
        }
        Ok(result)
    }
}
