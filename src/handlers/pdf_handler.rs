//! handlers/pdf_handler.rs
//! Endpoint para generar PDFs.

use std::path::PathBuf;

use actix_files::NamedFile;
use actix_web::{web, HttpResponse};
use log::error;

use crate::models::pdf_model::{PdfRequest, PdfResponse};
use crate::services::pdf_service::PdfService;

/// Recibe una petición POST con un JSON de tipo PdfRequest
/// y retorna un PDF binario en caso de éxito.
pub async fn generate_pdf_endpoint(
    pdf_service: web::Data<PdfService>,
    req_body: web::Json<PdfRequest>,
) -> HttpResponse {
    log::info!("Entrando a generate_pdf_endpoint");
    // Convertir web::Json<PdfRequest> a la estructura interna
    let file_name = req_body.file_name.clone();
    let req_data = req_body.into_inner();
    //log complete json

    // Llamar a la lógica de generación
    match pdf_service.generate_pdf(req_data).await {
        Ok(pdf_bytes) => {
            // Podríamos retornar un HttpResponse::Ok()
            // con header Content-Type: application/pdf
            HttpResponse::Ok()
                .append_header(("Content-Type", "application/pdf"))
                .append_header((
                    "Content-Disposition",
                    format!("inline; filename=\"{}\"", file_name),
                ))
                .append_header(("Cache-Control", "public, must-revalidate, max-age=0"))
                .append_header(("Pragma", "public"))
                .append_header(("Content-Length", pdf_bytes.len().to_string()))
                .body(pdf_bytes)
        }
        Err(e) => {
            error!("Error generando PDF: {:?}", e);
            HttpResponse::InternalServerError().json(PdfResponse {
                success: false,
                message: format!("Failed to generate PDF: {:?}", e),
            })
        }
    }
}

/// GET /api/pdf/local/{filename}
/// Sirve un archivo PDF que haya sido guardado en disco.
///
/// Ejemplo de URL: http://localhost:5022/api/pdf/local/XXXXX_document.pdf
pub async fn serve_local_pdf(path: web::Path<String>) -> Result<NamedFile, std::io::Error> {
    let filename = path.into_inner();
    // Carpeta donde guardamos los PDFs:
    let pdf_path = format!("./files/pdfs/{}", filename);

    // Actix Files gestiona los headers de Content-Type apropiados.
    // Retorna 404 si no existe.
    Ok(NamedFile::open(PathBuf::from(pdf_path))?)
}
