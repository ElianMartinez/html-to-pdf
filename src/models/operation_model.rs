use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationRecord {
    pub id: String,
    pub operation_type: String,
    pub status: String, // "pending", "running", "done", "failed"
    pub error_message: Option<String>,
    pub is_async: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub metadata: Option<String>, // JSON adicional
}

/// Request para crear una operación
#[derive(Debug, Clone, Deserialize)]
pub struct CreateOperationRequest {
    // define si es "send_email", "generate_pdf", etc.
    pub operation_type: String,
    pub is_async: bool,
    pub metadata: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreateOperationResponse {
    pub id: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct OperationStatusResponse {
    pub id: String,
    pub operation_type: String,
    pub status: String,
    pub error_message: Option<String>,
    pub is_async: bool,
    pub created_at: String,
    pub updated_at: String,
    pub metadata: Option<String>,
}

/// Para listar operaciones con paginación
#[derive(Debug, Clone, Serialize)]
pub struct ListOperationsResponse {
    pub total: u64,
    pub page: u64,
    pub page_size: u64,
    pub items: Vec<OperationStatusResponse>,
}
