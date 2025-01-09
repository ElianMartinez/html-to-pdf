//! config/pdf_config.rs
//! Estructuras globales para configuración de PDFs (dimensiones por defecto, etc.)

use serde::{Deserialize, Serialize};

/// Configuración global de PDF, con valores por defecto
/// (podría venir de un .toml, .env, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfGlobalConfig {
    pub default_orientation: String, // "portrait" o "landscape"
    pub default_width: f64,          // ancho por defecto (pulgadas)
    pub default_height: f64,         // alto por defecto
    pub default_margin_top: f64,
    pub default_margin_bottom: f64,
    pub default_margin_left: f64,
    pub default_margin_right: f64,
}

impl Default for PdfGlobalConfig {
    fn default() -> Self {
        PdfGlobalConfig {
            default_orientation: "portrait".to_string(),
            default_width: 8.5,
            default_height: 11.0,
            default_margin_top: 0.5,
            default_margin_bottom: 0.5,
            default_margin_left: 0.5,
            default_margin_right: 0.5,
        }
    }
}
