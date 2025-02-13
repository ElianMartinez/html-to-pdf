//! handlers/email_handler.rs

use actix_web::{web, HttpResponse};
use serde_json::json;

use crate::{
    models::{
        email_model::{EmailAttachment, SendUniversalEmailRequest},
        operation_model::CreateOperationRequest,
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

/// POST /api/email/send-unified
pub async fn send_universal_email_endpoint(
    email_service: web::Data<EmailService>,
    pdf_service: web::Data<PdfService>,
    _op_service: web::Data<OperationService>,
    body: web::Json<SendUniversalEmailRequest>,
) -> HttpResponse {
    let req_body = body.into_inner(); // Convertimos el JSON en struct
    let op_service_cloned = _op_service.clone();

    //log html for pdf
    if let Some(html) = &req_body.pdf_html {
        log::info!("HTML for PDF: {}", html);
    }

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

    let op_id = match op_service_cloned.create_operation(create_op_req).await {
        Ok(resp) => resp.id,
        Err(e) => {
            return HttpResponse::InternalServerError().json(json!({
                "success": false,
                "error": format!("Operation creation failed: {}", e)
            }))
        }
    };

    let email_service_cloned = email_service.clone();
    let pdf_service_cloned = pdf_service.clone();

    // 2. Decidir si se hace asíncrono o síncrono
    if req_body.async_send {
        // a) Asíncrono: lanzamos la tarea en background y devolvemos la respuesta de inmediato
        let op_id_clone = op_id.clone();
        let req_body_clone = req_body.clone();

        tokio::spawn(async move {
            if let Err(e) = do_full_email_work(
                op_id_clone.clone(),
                req_body_clone,
                email_service_cloned,
                pdf_service_cloned,
            )
            .await
            {
                // Marcamos la operación como fallida
                let _ = op_service_cloned
                    .mark_operation_failed(&op_id_clone, format!("Async task error: {:?}", e))
                    .await;
            } else {
                log::info!("Async email {} completado con éxito", op_id_clone);
            }
        });

        // Inmediatamente respondemos OK, sin haber bloqueado
        HttpResponse::Ok().json(json!({
            "success": true,
            "operation_id": op_id,
            "message": "Unified email queued for async processing"
        }))
    } else {
        // b) Síncrono: realizamos todo en el mismo hilo
        match do_full_email_work(
            op_id.clone(),
            req_body,
            email_service.clone(),
            pdf_service.clone(),
        )
        .await
        {
            Ok(_) => HttpResponse::Ok().json(json!({
                "success": true,
                "operation_id": op_id,
                "message": "Unified email processed successfully"
            })),
            Err(e) => {
                let _ = _op_service
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
}

/// Función auxiliar para generar PDF (si corresponde),
/// preparar adjuntos y llamar a `send_unified` en EmailService.
/// Retorna `Ok(())` si todo fue bien, o `Err(...)` si hubo problemas.
async fn do_full_email_work(
    op_id: String,
    mut req_body: SendUniversalEmailRequest,
    email_service: web::Data<EmailService>,
    pdf_service: web::Data<PdfService>,
) -> Result<(), anyhow::Error> {
    // 1. Lista de adjuntos final
    let mut final_attachments = vec![];

    // 2. ¿Generar PDF?
    if let Some(html) = req_body.pdf_html.take() {
        let pdf_request = PdfRequest {
            file_name: req_body
                .pdf_attachment_name
                .clone()
                .unwrap_or_else(|| "document.pdf".to_string()),
            html,
            orientation: req_body.pdf_orientation.clone(),
            page_size_preset: req_body.pdf_page_size_preset.clone(),
            custom_page_size: req_body.pdf_custom_page_size.clone(),
            margins: req_body.pdf_margins.clone(),
            scale: req_body.pdf_scale,
            store_local_pdf: Some(false),
        };

        let pdf_bytes = pdf_service
            .generate_pdf(pdf_request)
            .await
            .map_err(|e| anyhow::anyhow!("Error generando PDF: {}", e))?;

        final_attachments.push(EmailAttachment {
            filename: req_body
                .pdf_attachment_name
                .clone()
                .unwrap_or_else(|| "document.pdf".to_string()),
            content_type: "application/pdf".to_string(),
            data: pdf_bytes,
        });
    }

    // 3. Adjuntos adicionales
    if let Some(ref mut other_files) = req_body.other_attachments {
        final_attachments.append(other_files);
    }

    // 4. Llamar al método unificado de EmailService
    //    Esto insertará el registro en tabla `emails`, e iniciará el envío
    email_service
        .send_unified(op_id.clone(), req_body, final_attachments)
        .await
        .map_err(|e| anyhow::anyhow!("Error en send_unified: {}", e))?;

    // 5. Si llegamos aquí, todo OK
    Ok(())
}
