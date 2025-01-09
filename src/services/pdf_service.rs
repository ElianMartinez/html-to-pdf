//! services/pdf_service.rs
//! Contiene la lógica para generar PDFs con Chromium (usando chromiumoxide 0.7).

use crate::config::pdf_config::PdfGlobalConfig;
use crate::models::pdf_model::{PaperSize, PdfMargins, PdfRequest};
use anyhow::{anyhow, Context, Result};
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::page::PrintToPdfParams;
use chromiumoxide::Page;
use futures_util::StreamExt;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::task::JoinHandle;
use uuid::Uuid;

/// Estructura auxiliar para formatear duraciones (ms, s, microsegundos).
struct TimingInfo {
    duration: Duration,
}

impl TimingInfo {
    fn new(duration: Duration) -> Self {
        Self { duration }
    }

    fn format(&self) -> String {
        let micros = self.duration.as_micros();
        let millis = self.duration.as_millis();
        let secs = self.duration.as_secs();

        if secs > 0 {
            format!("{}.{:03} segundos", secs, millis % 1000)
        } else if millis > 0 {
            format!("{}.{:03} milisegundos", millis, micros % 1000)
        } else {
            format!("{} microsegundos", micros)
        }
    }
}

/// Servicio PDF que mantiene una instancia de Chromium
/// y permite generar PDFs a partir de HTML.
#[derive(Clone)]
pub struct PdfService {
    /// Navegador compartido (Chromium en modo headless).
    browser: Arc<Browser>,
    /// Tarea que maneja los eventos del navegador (hay que mantenerla viva).
    #[allow(dead_code)]
    browser_task: Arc<JoinHandle<()>>,
    /// Configuración global de PDF (por defecto).
    global_config: PdfGlobalConfig,
}

impl PdfService {
    /// Construye el servicio de forma asíncrona, lanzando Chromium con chromiumoxide.
    pub async fn new(global_config: PdfGlobalConfig) -> Result<Self> {
        let config = BrowserConfig::builder()
            .args(vec![
                "--headless",
                "--no-sandbox",
                "--disable-setuid-sandbox",
                "--disable-gpu",
                "--disable-software-rasterizer",
                "--disable-dev-shm-usage",
                "--disable-background-networking",
                "--disable-breakpad",
                "--disable-client-side-phishing-detection",
                "--disable-component-update",
                "--disable-default-apps",
                "--disable-extensions",
                "--disable-sync",
                "--disable-translate",
                "--metrics-recording-only",
                "--mute-audio",
                "--hide-scrollbars",
                "--disable-background-timer-throttling",
                "--disable-backgrounding-occluded-windows",
                "--disable-renderer-backgrounding",
                "--disable-zygote",
                "--disable-crash-reporter",
            ])
            .build()
            .map_err(|err_str| anyhow!("No se pudo construir BrowserConfig: {}", err_str))?;

        let (browser, mut handler) = Browser::launch(config).await.map_err(|err| {
            anyhow!(
                "No se pudo lanzar Chrome headless con chromiumoxide: {}",
                err
            )
        })?;

        let handle = tokio::spawn(async move {
            while let Some(event) = handler.next().await {
                log::debug!("Chromium event: {:?}", event);
            }
        });

        Ok(PdfService {
            browser: Arc::new(browser),
            browser_task: Arc::new(handle),
            global_config,
        })
    }

    // Constructor para tests
    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) async fn new_test(
        browser: Browser,
        handle: JoinHandle<()>,
        config: PdfGlobalConfig,
    ) -> Self {
        Self {
            browser: Arc::new(browser),
            browser_task: Arc::new(handle),
            global_config: config,
        }
    }

    /// Genera un PDF a partir de `PdfRequest`, devolviendo bytes PDF o error.
    /// GARANTIZA el cierre de la pestaña incluso si ocurre un error.
    pub async fn generate_pdf(&self, req: PdfRequest) -> Result<Vec<u8>> {
        // 1) Crear nueva pestaña
        let page_start = Instant::now();
        let page = self
            .browser
            .new_page("about:blank")
            .await
            .map_err(|err| anyhow!("No se pudo crear nueva página (tab): {}", err))?;

        log::info!(
            "⏱ Crear página: {}",
            TimingInfo::new(page_start.elapsed()).format()
        );

        // 2) Llamar a la lógica interna
        let result = self._generate_pdf_internal(&page, req).await;

        // 3) Cerrar la pestaña SIEMPRE
        let close_start = Instant::now();
        if let Err(err) = page.close().await {
            log::warn!("Error al cerrar la página: {}", err);
        }
        log::info!(
            "⏱ Cerrar página: {}",
            TimingInfo::new(close_start.elapsed()).format()
        );

        // 4) Retornar el resultado (Ok o Err)
        result
    }

    /// Lógica interna: navegar, esperar, imprimir PDF.
    /// Recibe una referencia a la pestaña para trabajar, y se cierra afuera.
    async fn _generate_pdf_internal(&self, page: &Page, req: PdfRequest) -> Result<Vec<u8>> {
        let start_total = Instant::now();
        let mut file_path = PathBuf::new();

        // 1. Preparar HTML
        let url_start = Instant::now();
        let html_str = req.html;
        let size_category = req.size_category.unwrap_or_else(|| "small".to_string());

        // Decidir data:text/html vs file://
        let final_url = if size_category == "large" {
            // Directorio temporal
            let tmp_dir = PathBuf::from("./tmp");
            fs::create_dir_all(&tmp_dir).context("No se pudo crear directorio ./tmp")?;

            let file_name = format!("temp_{}.html", Uuid::new_v4());
            file_path = tmp_dir.join(&file_name);

            fs::write(&file_path, &html_str).context("No se pudo escribir HTML en archivo")?;
            let absolute_path = file_path
                .canonicalize()
                .context("No se pudo obtener ruta absoluta")?;

            format!("file://{}", absolute_path.to_string_lossy())
        } else {
            let encoded = urlencoding::encode(&html_str);
            format!("data:text/html,{}", encoded)
        };

        log::info!(
            "⏱ Generar URL: {}",
            TimingInfo::new(url_start.elapsed()).format()
        );

        // 2. Navegar
        let nav_start = Instant::now();
        page.goto(final_url)
            .await
            .map_err(|err| anyhow!("Error al navegar al HTML (chromiumoxide): {}", err))?;
        log::info!(
            "⏱ Navegar a la URL: {}",
            TimingInfo::new(nav_start.elapsed()).format()
        );

        // 3. Esperar carga
        let wait_start = Instant::now();
        page.wait_for_navigation()
            .await
            .map_err(|err| anyhow!("Error esperando a que cargue la página: {}", err))?;
        log::info!(
            "⏱ Espera de carga: {}",
            TimingInfo::new(wait_start.elapsed()).format()
        );

        // 4. Configurar PrintToPdfParams
        let params_start = Instant::now();

        let paper_size: PaperSize = req.paper_size.unwrap_or_else(|| PaperSize {
            width: self.global_config.default_width,
            height: self.global_config.default_height,
        });

        let margins: PdfMargins = req.margins.unwrap_or_else(|| PdfMargins {
            top: self.global_config.default_margin_top,
            bottom: self.global_config.default_margin_bottom,
            left: self.global_config.default_margin_left,
            right: self.global_config.default_margin_right,
        });

        // Nota: la orientación no se usa en tu ejemplo
        // Si deseas, añade un .landscape(...) condicional
        let print_pdf = PrintToPdfParams::builder()
            .landscape(false)
            .paper_width(paper_size.width)
            .paper_height(paper_size.height)
            .margin_top(margins.top)
            .margin_bottom(margins.bottom)
            .margin_left(margins.left)
            .margin_right(margins.right)
            .print_background(true)
            .prefer_css_page_size(false)
            .scale(1.0)
            .display_header_footer(false)
            .build();

        log::info!(
            "⏱ Configurar parámetros: {}",
            TimingInfo::new(params_start.elapsed()).format()
        );

        // 5. Generar el PDF
        let pdf_start = Instant::now();
        let pdf_result = page
            .pdf(print_pdf)
            .await
            .map_err(|err| anyhow!("Error generando PDF: {}", err))?;

        log::info!(
            "⏱ Generar PDF: {}",
            TimingInfo::new(pdf_start.elapsed()).format()
        );

        // 6. Si se generó un archivo temporal, eliminarlo
        if size_category == "large" && file_path.exists() {
            if let Err(e) = fs::remove_file(&file_path) {
                log::warn!(
                    "No se pudo eliminar archivo temporal {:?}: {}",
                    &file_path,
                    e
                );
            }
        }

        // 7. Tiempo total
        let total_time = start_total.elapsed();
        log::info!(
            "⏱ Tiempo total de generación PDF: {}",
            TimingInfo::new(total_time).format()
        );

        Ok(pdf_result)
    }
}
