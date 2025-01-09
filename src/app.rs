//! app.rs
use crate::handlers::{email_handler, operation_handler, pdf_handler};
use actix_web::web;

pub fn init_app(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api")
            .service(
                web::scope("/pdf").route("", web::post().to(pdf_handler::generate_pdf_endpoint)),
            )
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
            .service(
                web::scope("/email")
                    .route("/send", web::post().to(email_handler::send_email_endpoint))
                    .route(
                        "/status/{op_id}",
                        web::get().to(email_handler::email_status_endpoint),
                    ),
            ),
    );
}
