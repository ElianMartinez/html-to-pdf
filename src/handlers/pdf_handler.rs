//! handlers/pdf_handler.rs
//! Endpoint para generar PDFs.

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
