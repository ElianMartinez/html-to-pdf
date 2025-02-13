use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationChannelRecord {
    pub id: String,
    pub operation_id: String,
    pub channel: String, // "email", "whatsapp", ...
    pub status: String,  // "pending", "running", "done", "failed"
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub attempts: i32,
}
