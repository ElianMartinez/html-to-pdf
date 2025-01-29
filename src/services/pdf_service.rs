use crate::models::pdf_model::{PdfMargins, PdfOrientation, PdfPagePreset, PdfRequest};
use anyhow::{anyhow, Context, Result};
use std::{
    fs,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    process::Command,
    sync::{Semaphore, SemaphorePermit},
    time::timeout,
};
use uuid::Uuid;

/// Ej: Manejo de concurrencia
const MAX_CONCURRENT_PROCESSES: usize = 8;
const PDF_GENERATION_TIMEOUT: Duration = Duration::from_secs(120);
const TEMP_DIR_PREFIX: &str = "pdf_service_";

#[derive(Clone)]
pub struct PdfService {
    semaphore: Arc<Semaphore>,
    temp_dir: Arc<PathBuf>,
    wkhtmltopdf_path: Arc<PathBuf>,
}

impl PdfService {
    pub async fn new() -> Result<Self> {
        // Creas un subdirectorio temporal
        let temp_dir = std::env::temp_dir().join(format!("{}_{}", TEMP_DIR_PREFIX, Uuid::new_v4()));
        fs::create_dir_all(&temp_dir)?;

        // Verifica la existencia de wkhtmltopdf
        let wkhtmltopdf_path =
            which::which("wkhtmltopdf").context("No se encontró wkhtmltopdf en el sistema")?;

        Ok(Self {
            semaphore: Arc::new(Semaphore::new(MAX_CONCURRENT_PROCESSES)),
            temp_dir: Arc::new(temp_dir),
            wkhtmltopdf_path: Arc::new(wkhtmltopdf_path),
        })
    }

    pub async fn generate_pdf(&self, req: PdfRequest) -> Result<Vec<u8>> {
        let start = Instant::now();
        // Adquirir permiso (control de concurrencia)
        let _guard = self.acquire_permit().await?;

        // Crear archivo HTML y PDF temporales
        let temp_files = self.create_temp_files(&req.file_name)?;
        let _cleanup = TempCleanup::new(temp_files.clone()); // para auto-borrar

        // Escribe el HTML a disco
        fs::write(&temp_files.html_path, &req.html).with_context(|| {
            format!(
                "Error escribiendo HTML temporal en {:?}",
                temp_files.html_path
            )
        })?;

        // Construimos el comando
        let pdf_data = self.run_wkhtmltopdf(&req, &temp_files).await?;

        let elapsed = start.elapsed().as_secs_f32();
        log::info!("PDF generado en {:.2}s", elapsed);
        Ok(pdf_data)
    }

    async fn acquire_permit(&self) -> Result<SemaphorePermit> {
        timeout(Duration::from_secs(5), self.semaphore.acquire())
            .await
            .context("Timeout esperando un permiso de PDFService")?
            .map_err(|_| anyhow!("No se pudo adquirir semaphore"))
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

    /// Aquí traducimos los parámetros del PdfRequest a flags de wkhtmltopdf
    async fn run_wkhtmltopdf(&self, req: &PdfRequest, paths: &TempFiles) -> Result<Vec<u8>> {
        // 1) Comenzamos con un Command
        let mut cmd = Command::new(&*self.wkhtmltopdf_path);

        // ========== ORIENTACIÓN ==========
        // Por default, "Portrait"
        let orientation = req
            .orientation
            .as_ref()
            .unwrap_or(&PdfOrientation::Portrait);
        let orientation_str = match orientation {
            PdfOrientation::Landscape => "Landscape",
            PdfOrientation::Portrait => "Portrait",
        };

        cmd.arg("--orientation").arg(orientation_str);

        // ========== TAMAÑO DE PÁGINA ==========
        // Si se especifica `page_size_preset` (A4, Letter, etc.), lo usamos:
        if let Some(preset) = &req.page_size_preset {
            let preset_str = match preset {
                PdfPagePreset::A4 => "A4",
                PdfPagePreset::Letter => "Letter",
                PdfPagePreset::Legal => "Legal",
                PdfPagePreset::A3 => "A3",
                PdfPagePreset::Tabloid => "Tabloid",
            };
            cmd.arg("--page-size").arg(preset_str);
        }
        // Caso contrario, si existe un tamaño personalizado:
        else if let Some(custom) = &req.custom_page_size {
            // Nota: wkhtmltopdf usa milímetros si se pasa a
            // --page-width/--page-height
            cmd.arg("--page-width").arg(format!("{}mm", custom.width));
            cmd.arg("--page-height").arg(format!("{}mm", custom.height));
        } else {
            // Si no se especificó nada, default a A4
            cmd.arg("--page-size").arg("A4");
        }

        // ========== MÁRGENES ==========
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

        // ========== ESCALA (zoom) ==========
        // wkhtmltopdf no tiene flag `--scale`, pero sí `--zoom`
        let scale = req.scale.unwrap_or(1.0);
        if (scale - 1.0).abs() > f64::EPSILON {
            cmd.arg("--zoom").arg(format!("{}", scale));
        }

        // ========== OTRAS OPCIONES GENÉRICAS ==========
        // Podrías poner más flags, p. ej.:
        //    --disable-smart-shrinking, --enable-local-file-access, etc.
        cmd.arg("--enable-local-file-access");
        cmd.arg("--print-media-type");

        // Importante: entradas y salida
        cmd.arg(&paths.html_path);
        cmd.arg(&paths.pdf_path);

        // 2) Configuramos redirecciones
        cmd.stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        // 3) Ejecutamos con timeout
        let output = timeout(PDF_GENERATION_TIMEOUT, cmd.output())
            .await
            .context("Timeout generando PDF con wkhtmltopdf")?
            .context("No se pudo ejecutar wkhtmltopdf")?;

        if !output.status.success() {
            let stderr_msg = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("wkhtmltopdf falló: {}", stderr_msg));
        }

        // 4) Cargamos el PDF generado
        let pdf_bytes = fs::read(&paths.pdf_path)
            .with_context(|| format!("Error leyendo PDF final en {:?}", paths.pdf_path))?;

        Ok(pdf_bytes)
    }
}

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

/// Se encarga de borrar los temporales al salir del scope
impl Drop for TempCleanup {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.files.html_path);
        let _ = fs::remove_file(&self.files.pdf_path);
    }
}
