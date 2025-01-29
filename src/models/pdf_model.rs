//! models/pdf_model.rs
//! Estructuras de datos para requests/responses de PDF

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct PdfMargins {
    pub top: f64,
    pub bottom: f64,
    pub left: f64,
    pub right: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PaperSize {
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PdfRequest {
    pub file_name: String,
    /// HTML que vamos a renderizar.
    pub html: String,

    /// Orientación (portrait o landscape)
    pub orientation: Option<String>,

    /// Dimensiones (en pulgadas) si el usuario desea personalizar.
    pub paper_size: Option<PaperSize>,

    /// Márgenes personalizados.
    pub margins: Option<PdfMargins>,

    /// Indica si es un PDF grande, mediano, etc. para quizás ajustar
    /// algo en la renderización (p.e., data URL vs. file://).
    pub size_category: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PdfResponse {
    pub success: bool,
    pub message: String,
}

impl PdfRequest {
    #[allow(dead_code)]
    #[cfg(test)]
    pub fn test_new(html: String) -> Self {
        Self {
            file_name: "test.pdf".to_string(),
            html,
            orientation: None,
            paper_size: None,
            margins: None,
            size_category: None,
        }
    }
}
