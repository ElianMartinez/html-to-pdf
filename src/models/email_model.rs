use crate::models::pdf_model::{PaperSize, PdfMargins};
use base64;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct SendEmailWithPdfRequest {
    // Email configuration
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_user: String,
    pub smtp_pass: String,
    pub recipient: String,
    pub subject: String,
    pub body: String,
    pub async_send: bool,

    // PDF configuration
    pub pdf_html: String,
    pub pdf_orientation: Option<String>,
    pub pdf_paper_size: Option<PaperSize>,
    pub pdf_margins: Option<PdfMargins>,
    pub pdf_size_category: Option<String>,

    // Attachment options
    pub pdf_attachment_name: Option<String>,
}

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

#[derive(Debug, Clone, Deserialize)]
pub struct SendEmailRequest {
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_user: String,
    pub smtp_pass: String,
    pub recipient: String,
    pub subject: String,
    pub body: String,
    pub async_send: bool,
    pub attachments: Option<Vec<EmailAttachment>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EmailStatusResponse {
    pub id: String,
    pub status: String,
    pub error: Option<String>,
}
