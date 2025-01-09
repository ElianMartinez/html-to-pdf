//! app.rs
//! Aquí definimos la función `init_app` que registra todas las rutas/endpoints
//! en la aplicación Actix.

use crate::handlers::pdf_handler;
use actix_web::web;

pub fn init_app(cfg: &mut web::ServiceConfig) {
    log::info!("Configurando rutas de la aplicación...");
    // Aquí agrupamos las rutas de cada "feature"
    cfg.service(
        web::scope("/api").service(
            web::scope("/pdf").route("", web::post().to(pdf_handler::generate_pdf_endpoint)),
        ), // .service(web::scope("/notifications").route(
           //     "/send",
           //     web::post().to(notification_handler::send_notification_endpoint),
           // ))
           // .service(
           //     web::scope("/email")
           //         .route("/send", web::post().to(email_handler::send_email_endpoint)),
           // ),
    );
}
