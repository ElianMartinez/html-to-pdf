//! models/email_model.rs

use base64;
use serde::{Deserialize, Serialize};

use crate::models::pdf_model::{PaperSize, PdfMargins, PdfOrientation, PdfPagePreset};

/// Representa un adjunto cualquiera (PDF, imagen, TXT, etc.) en base64.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailAttachment {
    pub filename: String,
    pub content_type: String,

    #[serde(
        serialize_with = "serialize_base64",
        deserialize_with = "deserialize_base64"
    )]
    pub data: Vec<u8>,
}

fn serialize_base64<S>(data: &[u8], serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&base64::encode(data))
}

fn deserialize_base64<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    base64::decode(&s).map_err(serde::de::Error::custom)
}

/// Request unificado para enviar correos con o sin PDF + adjuntos.
#[derive(Debug, Clone, Deserialize)]
pub struct SendUniversalEmailRequest {
    // Config SMTP
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_user: String,
    pub smtp_pass: String,

    // Destinatarios (uno o varios)
    pub recipients: Vec<String>,

    // Datos principales del correo
    pub subject: String,
    /// Cuerpo HTML
    pub body: String,
    /// Indica si el envío es asíncrono (true) o síncrono (false)
    pub async_send: bool,

    // ----------------------------
    // CAMPOS PARA GENERAR PDF
    // ----------------------------
    /// Si está presente, indica que queremos generar un PDF a partir de este HTML
    pub pdf_html: Option<String>,

    /// Orientación del PDF (portrait o landscape).
    pub pdf_orientation: Option<PdfOrientation>,

    /// Tamaño de página predefinido (A4, Letter, etc.)  
    pub pdf_page_size_preset: Option<PdfPagePreset>,

    /// Tamaño de página personalizado (si no se usa pdf_page_size_preset).
    pub pdf_custom_page_size: Option<PaperSize>,

    /// Márgenes para el PDF (en mm).
    pub pdf_margins: Option<PdfMargins>,

    /// Factor de escala (zoom); si es None, se asume 1.0
    pub pdf_scale: Option<f64>,

    /// Nombre con el que se adjuntará el PDF (por defecto: "document.pdf")
    pub pdf_attachment_name: Option<String>,

    // ----------------------------
    // OTROS ADJUNTOS
    // ----------------------------
    pub other_attachments: Option<Vec<EmailAttachment>>,
}

/// Respuesta al consultar estado de un email/operación
#[derive(Debug, Clone, Serialize)]
pub struct EmailStatusResponse {
    pub id: String,
    pub status: String,
    pub error: Option<String>,
}
