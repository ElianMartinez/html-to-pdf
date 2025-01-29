//! models/pdf_model.rs

use serde::{Deserialize, Serialize};

/// Márgenes en milímetros.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct PdfMargins {
    pub top: f64,
    pub bottom: f64,
    pub left: f64,
    pub right: f64,
}

/// Representa un tamaño personalizado en milímetros (ancho x alto).
#[derive(Debug, Clone, Deserialize)]
pub struct PaperSize {
    pub width: f64,
    pub height: f64,
}

/// Indica la orientación del PDF
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PdfOrientation {
    Portrait,
    Landscape,
}

/// Indica un tamaño predefinido (A4, Letter, etc.),
/// o "Custom" si el usuario prefiere anchura/altura en `custom_page_size`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum PdfPagePreset {
    A4,
    Letter,
    Legal,
    A3,
    Tabloid, // Agrega más si lo requieres (A3, Tabloid, etc.)
}

/// Request para generar PDF usando wkhtmltopdf
#[derive(Debug, Clone, Deserialize)]
pub struct PdfRequest {
    /// Nombre final (no necesariamente se usa en la salida, pero sí para logs)
    pub file_name: String,
    /// Contenido HTML a convertir
    pub html: String,

    /// Orientación (portrait o landscape). Si es `None`, se asume portrait
    pub orientation: Option<PdfOrientation>,

    /// Tamaño predefinido de página (A4, Letter, etc.)
    pub page_size_preset: Option<PdfPagePreset>,

    /// Si el usuario desea un tamaño personalizado (en mm).
    /// Se ignora si `page_size_preset` != None.
    pub custom_page_size: Option<PaperSize>,

    /// Márgenes (mm). Si es `None`, usar un default.
    pub margins: Option<PdfMargins>,

    /// Factor de escala (zoom). Ej: 1.0 (100%), 0.8 (80%), etc.
    /// Si es None, se asume 1.0
    pub scale: Option<f64>,
}

/// Respuesta genérica
#[derive(Debug, Clone, Serialize)]
pub struct PdfResponse {
    pub success: bool,
    pub message: String,
}

impl Default for PdfRequest {
    fn default() -> Self {
        Self {
            file_name: "output.pdf".to_string(),
            html: "".to_string(),
            orientation: Some(PdfOrientation::Portrait),
            page_size_preset: Some(PdfPagePreset::A4),
            custom_page_size: None,
            margins: Some(PdfMargins {
                top: 10.0,
                bottom: 10.0,
                left: 10.0,
                right: 10.0,
            }),
            scale: Some(1.0),
        }
    }
}
