//! handlers/operation_handler.rs
use actix_web::{web, HttpResponse};
use serde::Deserialize;

use crate::models::operation_model::CreateOperationRequest;
use crate::services::operation_service::OperationService;

#[derive(Deserialize)]
pub struct PaginationQuery {
    page: Option<u64>,
    page_size: Option<u64>,
}

/// POST /api/operations
pub async fn create_operation_endpoint(
    op_service: web::Data<OperationService>,
    body: web::Json<CreateOperationRequest>,
) -> HttpResponse {
    match op_service.create_operation(body.into_inner()).await {
        Ok(resp) => HttpResponse::Ok().json(resp),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": "Internal server error",
            "details": format!("{:?}", e)
        })),
    }
}

/// GET /api/operations
pub async fn list_operations_endpoint(
    op_service: web::Data<OperationService>,
    query: web::Query<PaginationQuery>,
) -> HttpResponse {
    let page = query.page.unwrap_or(1);
    let page_size = query.page_size.unwrap_or(10);

    match op_service.list_operations(page, page_size).await {
        Ok(list) => HttpResponse::Ok().json(list),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": "Internal server error",
            "details": format!("{:?}", e)
        })),
    }
}

/// GET /api/operations/{id}
pub async fn get_operation_endpoint(
    op_service: web::Data<OperationService>,
    path: web::Path<String>,
) -> HttpResponse {
    let op_id = path.into_inner();

    match op_service.get_operation(&op_id).await {
        Ok(op_record) => HttpResponse::Ok().json(op_record),
        Err(e) => HttpResponse::NotFound().json(serde_json::json!({
            "error": "Operation not found",
            "details": format!("{:?}", e)
        })),
    }
}
