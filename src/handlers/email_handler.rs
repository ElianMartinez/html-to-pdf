//! handlers/email_handler.rs

use actix_web::{web, HttpResponse};
use serde_json::json;

use crate::{
    models::{
        email_model::SendUniversalEmailRequest, operation_model::CreateOperationRequest,
        pdf_model::PdfRequest,
    },
    services::{
        email_service::EmailService, operation_service::OperationService, pdf_service::PdfService,
    },
};

/// GET /api/email/status/{op_id}
/// Devuelve el estado de un envío de correo (por operation_id).
pub async fn email_status_endpoint(
    email_service: web::Data<EmailService>,
    path: web::Path<String>,
) -> HttpResponse {
    let op_id = path.into_inner();

    match email_service.get_email_status(&op_id).await {
        Ok(status) => HttpResponse::Ok().json(json!({
            "success": true,
            "status": status
        })),
        Err(e) => {
            let status_code = if e.to_string().contains("not found") {
                actix_web::http::StatusCode::NOT_FOUND
            } else {
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR
            };

            HttpResponse::build(status_code).json(json!({
                "success": false,
                "error": e.to_string()
            }))
        }
    }
}

// =====================================================================================================
// ENDPOINT UNIFICADO: POST /api/email/send-unified
// Maneja:
//  - Envío a múltiples destinatarios
//  - Generación PDF opcional
//  - Adjuntos opcionales
//  - Síncrono o asíncrono
// =====================================================================================================
pub async fn send_universal_email_endpoint(
    email_service: web::Data<EmailService>,
    pdf_service: web::Data<PdfService>,
    op_service: web::Data<OperationService>,
    body: web::Json<SendUniversalEmailRequest>,
) -> HttpResponse {
    let mut req_body = body.into_inner();

    // 1. Crear la operación
    let create_op_req = CreateOperationRequest {
        operation_type: "send_unified_email".to_string(),
        is_async: req_body.async_send,
        metadata: Some(
            json!({
                "recipients": req_body.recipients,
                "subject": req_body.subject,
                "pdf_planned": req_body.pdf_html.is_some(),
                "other_attachments": req_body.other_attachments.as_ref().map(|a| a.len()).unwrap_or(0)
            })
            .to_string(),
        ),
    };

    let op_id = match op_service.create_operation(create_op_req).await {
        Ok(resp) => resp.id,
        Err(e) => {
            return HttpResponse::InternalServerError().json(json!({
                "success": false,
                "error": format!("Operation creation failed: {}", e)
            }))
        }
    };

    // 2. Revisar si hay que generar PDF
    let mut final_attachments = vec![];

    if let Some(html) = req_body.pdf_html.take() {
        // Construir PdfRequest usando el nuevo modelo
        let pdf_request = PdfRequest {
            file_name: req_body
                .pdf_attachment_name
                .clone()
                .unwrap_or_else(|| "document.pdf".to_string()),
            html,
            orientation: req_body.pdf_orientation.clone(), // Add .clone() here
            page_size_preset: req_body.pdf_page_size_preset.clone(),
            custom_page_size: req_body.pdf_custom_page_size.clone(),
            margins: req_body.pdf_margins.clone(),
            scale: req_body.pdf_scale.clone(), // Add .clone() here if scale is not Copy
        };

        // Llamamos a pdf_service
        let pdf_bytes = match pdf_service.generate_pdf(pdf_request).await {
            Ok(bytes) => bytes,
            Err(e) => {
                // Marcamos la operación como fallida
                let _ = op_service
                    .mark_operation_failed(&op_id, format!("PDF generation failed: {}", e))
                    .await;
                return HttpResponse::InternalServerError().json(json!({
                    "success": false,
                    "operation_id": op_id,
                    "error": format!("PDF Error: {}", e)
                }));
            }
        };

        // Agregamos el PDF a la lista de adjuntos
        final_attachments.push(crate::models::email_model::EmailAttachment {
            filename: req_body
                .pdf_attachment_name
                .as_ref()
                .map(|s| s.clone())
                .unwrap_or_else(|| "document.pdf".to_string()),
            content_type: "application/pdf".to_string(),
            data: pdf_bytes,
        });
    }

    // 3. Adjuntos adicionales
    if let Some(ref mut other_files) = req_body.other_attachments {
        final_attachments.append(other_files);
    }

    // 4. Llamar al servicio unificado
    match email_service
        .send_unified(op_id.clone(), req_body, final_attachments)
        .await
    {
        Ok(_) => HttpResponse::Ok().json(json!({
            "success": true,
            "operation_id": op_id,
            "message": "Unified email queued/sent successfully"
        })),
        Err(e) => {
            // Marcar operación fallida en caso de error
            let _ = op_service
                .mark_operation_failed(&op_id, format!("Email send failed: {}", e))
                .await;

            HttpResponse::InternalServerError().json(json!({
                "success": false,
                "operation_id": op_id,
                "error": format!("Email Error: {}", e)
            }))
        }
    }
}
