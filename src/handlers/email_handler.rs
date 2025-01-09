use crate::models::email_model::SendEmailRequest;
use crate::services::email_service::EmailService;
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
            "message": "Operación creada"
        })),
        Err(e) => HttpResponse::InternalServerError().json(json!({
            "success": false,
            "error": format!("{:?}", e)
        })),
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
        Err(e) => HttpResponse::NotFound().json(json!({
            "success": false,
            "error": format!("{:?}", e)
        })),
    }
}
