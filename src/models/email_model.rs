// models/email_model.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct SendEmailRequest {
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_user: String,
    pub smtp_pass: String,
    pub recipient: String,
    pub subject: String,
    pub body: String,
    pub async_send: bool, // si es true => asíncrono, false => síncrono
}

#[derive(Debug, Clone, Serialize)]
pub struct EmailStatusResponse {
    pub id: String,
    pub status: String,
    pub error: Option<String>,
}
