use actix_web::web;

use crate::handlers::{email_handler, notification_handler, operation_handler, pdf_handler};

pub fn init_app(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api")
            // Rutas PDF
            .service(
                web::scope("/pdf").route("", web::post().to(pdf_handler::generate_pdf_endpoint)),
            )
            // Rutas de operaciones
            .service(
                web::scope("/operations")
                    .route(
                        "",
                        web::post().to(operation_handler::create_operation_endpoint),
                    )
                    .route(
                        "",
                        web::get().to(operation_handler::list_operations_endpoint),
                    )
                    .route(
                        "/{id}",
                        web::get().to(operation_handler::get_operation_endpoint),
                    ),
            )
            // Rutas de email
            .service(
                web::scope("/email")
                    .route(
                        "/send-unified",
                        web::post().to(email_handler::send_universal_email_endpoint),
                    )
                    .route(
                        "/status/{op_id}",
                        web::get().to(email_handler::email_status_endpoint),
                    ),
            )
            // Rutas de notificaciones unificadas
            .service(web::scope("/notifications").route(
                "/send",
                web::post().to(notification_handler::send_unified_notification_endpoint),
            )),
    );
}
