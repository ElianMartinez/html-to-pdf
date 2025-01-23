use crate::{
    models::email_model::{SendEmailRequest, SendEmailWithPdfRequest},
    services::operation_service::OperationService,
    services::{email_service::EmailService, pdf_service::PdfService},
};
use actix_web::{web, HttpResponse};
use serde_json::json;

/// POST /api/email/send
pub async fn send_email_endpoint(
    email_service: web::Data<EmailService>,
    body: web::Json<SendEmailRequest>,
) -> HttpResponse {
    let req_data = body.into_inner();

    match email_service.send_email(req_data).await {
        Ok(op_id) => HttpResponse::Ok().json(json!({
            "success": true,
            "operation_id": op_id,
            "message": "Email processing started"
        })),
        Err(e) => {
            log::error!("Email send error: {}", e);
            HttpResponse::InternalServerError().json(json!({
                "success": false,
                "error": e.to_string()
            }))
        }
    }
}

/// POST /api/email/send-with-pdf
pub async fn send_email_with_pdf_endpoint(
    email_service: web::Data<EmailService>,
    pdf_service: web::Data<PdfService>,
    op_service: web::Data<OperationService>,
    body: web::Json<SendEmailWithPdfRequest>,
) -> HttpResponse {
    let body_data = body.into_inner();

    // 1. Crear operación combinada
    let create_op_req = crate::models::operation_model::CreateOperationRequest {
        operation_type: "email_with_pdf".to_string(),
        is_async: body_data.async_send,
        metadata: Some(
            json!({
                "recipient": body_data.recipient,
                "subject": body_data.subject,
                "pdf_attachment": body_data.pdf_attachment_name
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

    // 2. Generar PDF
    let pdf_request = crate::models::pdf_model::PdfRequest {
        html: body_data.pdf_html,
        orientation: body_data.pdf_orientation,
        paper_size: body_data.pdf_paper_size,
        margins: body_data.pdf_margins,
        size_category: body_data.pdf_size_category,
    };

    let pdf_bytes = match pdf_service.generate_pdf(pdf_request).await {
        Ok(bytes) => bytes,
        Err(e) => {
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

    // 3. Crear adjunto
    let attachment = crate::models::email_model::EmailAttachment {
        filename: body_data
            .pdf_attachment_name
            .unwrap_or_else(|| "document.pdf".to_string()),
        content_type: "application/pdf".to_string(),
        data: pdf_bytes,
    };

    // 4. Construir y enviar email
    let email_req = SendEmailRequest {
        smtp_host: body_data.smtp_host,
        smtp_port: body_data.smtp_port,
        smtp_user: body_data.smtp_user,
        smtp_pass: body_data.smtp_pass,
        recipient: body_data.recipient,
        subject: body_data.subject,
        body: body_data.body,
        async_send: body_data.async_send,
        attachments: Some(vec![attachment]),
    };

    match email_service.send_email(email_req).await {
        Ok(_) => HttpResponse::Ok().json(json!({
            "success": true,
            "operation_id": op_id,
            "message": "Email with PDF queued successfully"
        })),
        Err(e) => {
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

/// GET /api/email/status/{op_id}
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
