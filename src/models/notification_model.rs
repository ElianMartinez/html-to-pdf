use crate::models::{
    email_model::EmailAttachment,
    pdf_model::{PaperSize, PdfMargins, PdfOrientation, PdfPagePreset},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct NotificationRequest {
    /// ["email", "whatsapp"] etc.
    pub channels: Vec<String>,

    /// Config opcional para Email
    pub email_config: Option<EmailConfig>,

    /// Config opcional para WhatsApp
    pub whatsapp_config: Option<WhatsAppConfig>,

    /// Texto genérico
    pub subject: Option<String>,
    pub body: Option<String>,

    /// Indica si el envío es asíncrono
    pub async_send: bool,

    // PDF
    pub pdf_html: Option<String>,
    pub pdf_orientation: Option<PdfOrientation>,
    pub pdf_page_size_preset: Option<PdfPagePreset>,
    pub pdf_custom_page_size: Option<PaperSize>,
    pub pdf_margins: Option<PdfMargins>,
    pub pdf_scale: Option<f64>,
    pub pdf_attachment_name: Option<String>,

    // Adjuntos
    pub other_attachments: Option<Vec<EmailAttachment>>,
}

/// Config de email
#[derive(Debug, Clone, Deserialize)]
pub struct EmailConfig {
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_user: String,
    pub smtp_pass: String,
    pub recipients: Vec<String>,
}

/// Config de WhatsApp
#[derive(Debug, Clone, Deserialize)]
pub struct WhatsAppConfig {
    pub recipients: Vec<String>,
}

/// Respuesta genérica
#[derive(Debug, Clone, Serialize)]
pub struct NotificationResponse {
    pub success: bool,
    pub operation_id: String,
    pub message: String,
}
