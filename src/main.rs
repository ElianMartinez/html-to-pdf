use actix_web::{web, App, HttpServer};
use dotenv::dotenv;
use sqlx::{Pool, Sqlite};

use crate::config::pdf_config::PdfGlobalConfig;
use crate::logger::init_logger;
use crate::services::email_service::EmailService;
use crate::services::operation_service::OperationService;
use crate::services::pdf_service::PdfService;

mod app;
mod config;
mod handlers;
mod logger;
mod models;
mod services;
mod tests;

async fn setup_database() -> Pool<Sqlite> {
    // 1) Crear carpeta "data"
    std::fs::create_dir_all("data").expect("No se pudo crear directorio 'data'");

    // 2) Ruta final: ./data/operations.db
    let db_path = std::env::current_dir()
        .expect("No se pudo obtener el current_dir")
        .join("data")
        .join("operations.db");
    let db_url = format!("sqlite:{}", db_path.to_string_lossy());

    log::info!("Conectando a SQLite en {}", db_url);

    // 3) Conectarnos con SQLx
    let db_pool = Pool::<Sqlite>::connect(&db_url)
        .await
        .expect("No se pudo conectar a la base de datos SQLite");

    db_pool
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok(); // Cargar .env al inicio
    init_logger();

    let pdf_service = PdfService::new()
        .await
        .expect("No se pudo inicializar PdfService");

    // Conectarnos a la DB
    let db_pool = setup_database().await;

    // Verificar la conexión
    let conn = db_pool.acquire().await.expect("Falló la conexión");
    drop(conn);

    // OperationService
    let operation_service = OperationService::new(db_pool.clone());
    if let Err(e) = operation_service.run_migrations().await {
        panic!("Fallo en migraciones de 'operations': {:?}", e);
    }

    // EmailService
    let email_service = EmailService::new(db_pool.clone(), operation_service.clone());
    if let Err(e) = email_service.run_migrations().await {
        panic!("Fallo en migraciones de 'emails': {:?}", e);
    }

    // Levantar servidor
    log::info!("Levantando servidor en 0.0.0.0:5022");
    HttpServer::new(move || {
        App::new()
            // Aumentar límite si recibes JSON muy grandes
            .app_data(web::JsonConfig::default().limit(500 * 1024 * 1024))
            .app_data(web::Data::new(pdf_service.clone()))
            .app_data(web::Data::new(operation_service.clone()))
            .app_data(web::Data::new(email_service.clone()))
            .configure(app::init_app)
    })
    .workers(1)
    .bind(("0.0.0.0", 5022))?
    .run()
    .await
}
