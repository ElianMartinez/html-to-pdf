use crate::models::pdf_model::{PdfMargins, PdfOrientation, PdfPagePreset, PdfRequest};
use anyhow::{anyhow, Context, Result};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    process::Command,
    sync::{Semaphore, SemaphorePermit},
    time::timeout,
};
use uuid::Uuid;

/// Cantidad máxima de wkhtmltopdf simultáneos
const MAX_CONCURRENT_PROCESSES: usize = 8;
/// Tiempo máximo para generar un PDF
const PDF_GENERATION_TIMEOUT: Duration = Duration::from_secs(300);
/// Prefijo de carpeta temporal
const TEMP_DIR_PREFIX: &str = "pdf_service_";

#[derive(Clone)]
pub struct PdfService {
    semaphore: Arc<Semaphore>,
    temp_dir: Arc<PathBuf>,
    wkhtmltopdf_path: Arc<PathBuf>,
}

impl PdfService {
    pub async fn new() -> Result<Self> {
        // Crea un subdirectorio temporal (para HTML/PDF provisionales).
        let temp_dir = std::env::temp_dir().join(format!("{}_{}", TEMP_DIR_PREFIX, Uuid::new_v4()));
        fs::create_dir_all(&temp_dir)?;

        // Verifica que wkhtmltopdf esté en PATH
        let wkhtmltopdf_path =
            which::which("wkhtmltopdf").context("No se encontró wkhtmltopdf en el sistema")?;

        Ok(Self {
            semaphore: Arc::new(Semaphore::new(MAX_CONCURRENT_PROCESSES)),
            temp_dir: Arc::new(temp_dir),
            wkhtmltopdf_path: Arc::new(wkhtmltopdf_path),
        })
    }

    /// Genera un PDF en memoria (Vec<u8>).
    /// Si `req.store_local_pdf == Some(true)`, además se guarda localmente en ./files/pdfs/
    pub async fn generate_pdf(&self, req: PdfRequest) -> Result<Vec<u8>> {
        let start = Instant::now();

        // Control de concurrencia
        let _guard = self.acquire_permit().await?;

        // Crea archivos temporales (HTML y PDF)
        let temp_files = self.create_temp_files(&req.file_name)?;
        let _cleanup = TempCleanup::new(temp_files.clone()); // al final se borran

        // Escribir HTML a disco
        fs::write(&temp_files.html_path, &req.html).with_context(|| {
            format!(
                "Error escribiendo HTML temporal en {:?}",
                temp_files.html_path
            )
        })?;

        // Llamar a wkhtmltopdf
        let pdf_data = self.run_wkhtmltopdf(&req, &temp_files).await?;

        // Si el usuario quiere guardarlo localmente, lo hacemos ahora
        if req.store_local_pdf.unwrap_or(false) {
            // Creamos la carpeta si no existe
            let _ = fs::create_dir_all("./files/pdfs");

            // Generamos un nombre único: "<uuid>_<nombreOriginal>.pdf"
            let unique_name = format!("{}_{}", req.file_name, Uuid::new_v4());

            let local_path = Path::new("./files/pdfs").join(&unique_name);
            fs::write(&local_path, &pdf_data)
                .with_context(|| format!("No se pudo guardar PDF en {:?}", local_path))?;

            log::info!(
                "PDF guardado localmente en {:?} ({} bytes)",
                local_path,
                pdf_data.len()
            );
        }

        let elapsed = start.elapsed().as_secs_f32();
        log::info!("PDF generado en {:.2}s", elapsed);

        // Retornamos los bytes en memoria (útil si vas a adjuntarlos por email, etc.)
        Ok(pdf_data)
    }

    async fn acquire_permit(&self) -> Result<SemaphorePermit> {
        timeout(Duration::from_secs(5), self.semaphore.acquire())
            .await
            .context("Timeout esperando permiso en PdfService")?
            .map_err(|_| anyhow!("No se pudo adquirir el semaphore"))
    }

    fn create_temp_files(&self, base_name: &str) -> Result<TempFiles> {
        let id = Uuid::new_v4().to_string();
        let html_path = self.temp_dir.join(format!("{}_{}.html", base_name, id));
        let pdf_path = self.temp_dir.join(format!("{}_{}.pdf", base_name, id));
        Ok(TempFiles {
            html_path,
            pdf_path,
        })
    }

    async fn run_wkhtmltopdf(&self, req: &PdfRequest, paths: &TempFiles) -> Result<Vec<u8>> {
        let mut cmd = Command::new(&*self.wkhtmltopdf_path);

        // ===== ORIENTACIÓN =====
        let orientation = req
            .orientation
            .as_ref()
            .unwrap_or(&PdfOrientation::Portrait);
        let orientation_str = match orientation {
            PdfOrientation::Landscape => "Landscape",
            PdfOrientation::Portrait => "Portrait",
        };
        cmd.arg("--orientation").arg(orientation_str);

        // ===== TAMAÑO DE PÁGINA =====
        if let Some(preset) = &req.page_size_preset {
            let preset_str = match preset {
                PdfPagePreset::A4 => "A4",
                PdfPagePreset::Letter => "Letter",
                PdfPagePreset::Legal => "Legal",
                PdfPagePreset::A3 => "A3",
                PdfPagePreset::Tabloid => "Tabloid",
            };
            cmd.arg("--page-size").arg(preset_str);
        } else if let Some(custom) = &req.custom_page_size {
            cmd.arg("--page-width").arg(format!("{}mm", custom.width));
            cmd.arg("--page-height").arg(format!("{}mm", custom.height));
        } else {
            // Default a A4
            cmd.arg("--page-size").arg("A4");
        }

        // ===== MÁRGENES =====
        let margins = req.margins.as_ref().unwrap_or(&PdfMargins {
            top: 10.0,
            bottom: 10.0,
            left: 10.0,
            right: 10.0,
        });
        cmd.arg("--margin-top").arg(format!("{}mm", margins.top));
        cmd.arg("--margin-bottom")
            .arg(format!("{}mm", margins.bottom));
        cmd.arg("--margin-left").arg(format!("{}mm", margins.left));
        cmd.arg("--margin-right")
            .arg(format!("{}mm", margins.right));

        // ===== ESCALA (zoom) =====
        let scale = req.scale.unwrap_or(1.0);
        if (scale - 1.0).abs() > f64::EPSILON {
            cmd.arg("--zoom").arg(format!("{}", scale));
        }

        // ===== OTRAS OPCIONES =====
        cmd.arg("--enable-local-file-access");
        cmd.arg("--print-media-type");

        // Entradas/salidas
        cmd.arg(&paths.html_path);
        cmd.arg(&paths.pdf_path);

        cmd.stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let output = timeout(PDF_GENERATION_TIMEOUT, cmd.output())
            .await
            .context("Timeout ejecutando wkhtmltopdf")?
            .context("No se pudo lanzar wkhtmltopdf")?;

        if !output.status.success() {
            let stderr_msg = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("wkhtmltopdf falló: {}", stderr_msg));
        }

        // Leer el PDF final
        let pdf_bytes = fs::read(&paths.pdf_path)
            .with_context(|| format!("Error leyendo PDF final en {:?}", paths.pdf_path))?;

        Ok(pdf_bytes)
    }
}

// --------------------------------------------------------------------------------
// Estructuras auxiliares
// --------------------------------------------------------------------------------
#[derive(Clone)]
struct TempFiles {
    html_path: PathBuf,
    pdf_path: PathBuf,
}

struct TempCleanup {
    files: TempFiles,
}

impl TempCleanup {
    fn new(files: TempFiles) -> Self {
        Self { files }
    }
}

/// Borra los archivos temporales al salir de scope
impl Drop for TempCleanup {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.files.html_path);
        let _ = fs::remove_file(&self.files.pdf_path);
    }
}
