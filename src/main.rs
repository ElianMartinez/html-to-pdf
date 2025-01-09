//! main.rs

mod app; // define la función que construye la App de Actix
mod config; // módulos relacionados con configuración
mod errors; // manejo de errores globales
mod handlers; // endpoints
mod logger; // setup de logs
mod models; // estructuras/Modelos de datos
mod services; // lógica de negocio (PDF, etc.)

mod tests; // tests integrados (opcional)

use crate::config::pdf_config::PdfGlobalConfig;
use crate::logger::init_logger;
use actix_web::{App, HttpServer};
use std::io;

#[actix_web::main]
async fn main() -> io::Result<()> {
    // 1. Inicializar logs
    init_logger();

    log::info!("Cargando configuración PDF por defecto...");
    let pdf_config = PdfGlobalConfig::default();

    log::info!("Iniciando instancia de Chrome (chromiumoxide)...");
    // 2. Crear el servicio de PDF (asincrónico)
    //    NOTA: el "new" en chromiumoxide es async.
    let pdf_service = services::pdf_service::PdfService::new(pdf_config)
        .await
        .expect("No se pudo inicializar PdfService");

    // 3. Levantar el servidor
    log::info!("Levantando servidor en 127.0.0.1:5022");
    HttpServer::new(move || {
        App::new()
            .app_data(actix_web::web::JsonConfig::default().limit(20 * 1024 * 1024))
            // .app_data(...) con pdf_service (que implementa Clone)
            .app_data(actix_web::web::Data::new(pdf_service.clone()))
            .configure(app::init_app)
    })
    .workers(1)
    .bind(("127.0.0.1", 5022))?
    .run()
    .await
}
