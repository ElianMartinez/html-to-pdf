use anyhow::{Context, Result};
use chrono::Utc;
use sqlx::{Pool, Sqlite};
use uuid::Uuid;

use crate::models::operation_model::{
    CreateOperationRequest, CreateOperationResponse, ListOperationsResponse, OperationRecord,
    OperationStatusResponse,
};

#[derive(Clone, Debug)]
pub struct OperationService {
    db_pool: Pool<Sqlite>,
}

impl OperationService {
    pub fn new(db_pool: Pool<Sqlite>) -> Self {
        OperationService { db_pool }
    }

    /// Corre migraciones con sqlx
    pub async fn run_migrations(&self) -> Result<()> {
        sqlx::migrate!("./migrations").run(&self.db_pool).await?;
        Ok(())
    }

    /// Crea la operación en DB con estado "pending"
    /// Espera un `CreateOperationRequest` completo.
    pub async fn create_operation(
        &self,
        req: CreateOperationRequest,
    ) -> Result<CreateOperationResponse> {
        let op_id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        let is_async_int = req.is_async as i32;
        sqlx::query!(
            r#"
            INSERT INTO operations (
                id, operation_type, status, error_message,
                is_async, created_at, updated_at, metadata
            )
            VALUES (?1, ?2, 'pending', NULL, ?3, ?4, ?4, ?5)
            "#,
            op_id,
            req.operation_type,
            is_async_int,
            now,
            req.metadata
        )
        .execute(&self.db_pool)
        .await
        .context("Fallo al insertar operation")?;

        Ok(CreateOperationResponse {
            id: op_id,
            message: "Operación creada".to_string(),
        })
    }

    /// Actualiza estado y error
    pub async fn update_operation(
        &self,
        op_id: &str,
        new_status: &str,
        error_message: Option<&str>,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        sqlx::query!(
            r#"
            UPDATE operations
            SET status = ?2,
                error_message = ?3,
                updated_at = ?4
            WHERE id = ?1
            "#,
            op_id,
            new_status,
            error_message,
            now
        )
        .execute(&self.db_pool)
        .await
        .context("Fallo al actualizar operación")?;

        Ok(())
    }

    /// Obtiene la info de una operación
    pub async fn get_operation(&self, op_id: &str) -> Result<OperationRecord> {
        let row = sqlx::query!(
            r#"
            SELECT
                id, operation_type, status, error_message,
                is_async, created_at, updated_at, metadata
            FROM operations
            WHERE id = ?1
            "#,
            op_id
        )
        .fetch_one(&self.db_pool)
        .await
        .context("No se encontró operación con ese id")?;

        // parsea strings a boolean e ISO8601
        Ok(OperationRecord {
            id: row.id.unwrap_or_default(),
            operation_type: row.operation_type,
            status: row.status,
            error_message: row.error_message,
            is_async: row.is_async != 0,
            created_at: row.created_at.parse()?,
            updated_at: row.updated_at.parse()?,
            metadata: row.metadata,
        })
    }

    /// Lista operaciones con paginación
    pub async fn list_operations(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<ListOperationsResponse> {
        let offset = (page - 1) * page_size;
        let page_size_i64 = page_size as i64;
        let offset_i64 = offset as i64;

        // total
        let total_row = sqlx::query!("SELECT COUNT(*) as cnt FROM operations")
            .fetch_one(&self.db_pool)
            .await?;
        let total = total_row.cnt as u64;

        // items
        let rows = sqlx::query!(
            r#"
            SELECT
                id, operation_type, status, error_message,
                is_async, created_at, updated_at, metadata
            FROM operations
            ORDER BY created_at DESC
            LIMIT ?1 OFFSET ?2
            "#,
            page_size_i64,
            offset_i64
        )
        .fetch_all(&self.db_pool)
        .await?;

        let items: Vec<_> = rows
            .into_iter()
            .map(|r| OperationStatusResponse {
                id: r.id.unwrap_or_default(),
                operation_type: r.operation_type,
                status: r.status,
                error_message: r.error_message,
                is_async: r.is_async != 0,
                created_at: r.created_at,
                updated_at: r.updated_at,
                metadata: r.metadata,
            })
            .collect();

        Ok(ListOperationsResponse {
            total,
            page,
            page_size,
            items,
        })
    }

    pub async fn mark_operation_failed(
        &self,
        op_id: &str,
        error: String,
    ) -> Result<(), anyhow::Error> {
        sqlx::query!(
            r#"UPDATE operations 
            SET status = 'failed', 
                error_message = ?, 
                updated_at = CURRENT_TIMESTAMP 
            WHERE id = ?"#,
            error,
            op_id
        )
        .execute(&self.db_pool)
        .await?;

        Ok(())
    }

    pub async fn update_operation_status(
        &self,
        operation_id: &str,
        status: &str,
        error: Option<&str>,
    ) -> Result<()> {
        sqlx::query!(
            r#"UPDATE operations SET status = ?1, error_message = ?2 WHERE id = ?3"#,
            status,
            error,
            operation_id
        )
        .execute(&self.db_pool)
        .await
        .context("Failed to update operation status")?;
        Ok(())
    }
}
