use actix_web::{web, HttpResponse};
use serde_json::json;

use crate::{
    models::{
        notification_model::{NotificationRequest, NotificationResponse},
        operation_model::CreateOperationRequest,
    },
    services::{notification_service::NotificationService, operation_service::OperationService},
};

/// POST /api/notifications/send
pub async fn send_unified_notification_endpoint(
    body: web::Json<NotificationRequest>,
    notification_service: web::Data<NotificationService>,
    operation_service: web::Data<OperationService>,
) -> HttpResponse {
    let req_body = body.into_inner();
    let op_service_cloned = operation_service.clone();

    // Crear la operación
    let create_op_req = CreateOperationRequest {
        operation_type: "send_notification".to_string(),
        is_async: req_body.async_send,
        metadata: Some(
            serde_json::json!({
                "channels": req_body.channels,
                "pdf_planned": req_body.pdf_html.is_some(),
                "has_attachments": req_body.other_attachments.as_ref().map(|v| v.len()).unwrap_or(0)
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

    // Asíncrono o síncrono
    if req_body.async_send {
        let op_id_clone = op_id.clone();
        let req_clone = req_body.clone();
        let service_clone = notification_service.clone();

        tokio::spawn(async move {
            if let Err(e) = service_clone
                .process_notification(op_id_clone.clone(), req_clone)
                .await
            {
                let _ = op_service_cloned
                    .mark_operation_failed(&op_id_clone, format!("Async task error: {:?}", e))
                    .await;
            }
        });

        HttpResponse::Ok().json(NotificationResponse {
            success: true,
            operation_id: op_id,
            message: "Notification queued for async processing".to_string(),
        })
    } else {
        match notification_service
            .process_notification(op_id.clone(), req_body)
            .await
        {
            Ok(_) => HttpResponse::Ok().json(NotificationResponse {
                success: true,
                operation_id: op_id,
                message: "Notification processed successfully".to_string(),
            }),
            Err(e) => {
                let _ = operation_service
                    .mark_operation_failed(&op_id, format!("Send failed: {}", e))
                    .await;
                HttpResponse::InternalServerError().json(json!({
                    "success": false,
                    "operation_id": op_id,
                    "error": format!("{:?}", e)
                }))
            }
        }
    }
}
