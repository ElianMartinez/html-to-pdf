use actix_web::{
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    web, App, Error, HttpResponse, HttpServer,
};
use dotenv::dotenv;
use serde_json::json;
use services::notification_channel_service::NotificationChannelService;
use services::notification_service::NotificationService;
use sqlx::{Pool, Sqlite};

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

pub struct ApiKeyMiddleware;

impl<S> Transform<S, ServiceRequest> for ApiKeyMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse, Error = Error> + 'static,
{
    type Response = ServiceResponse;
    type Error = Error;
    type InitError = ();
    type Transform = ApiKeyMiddlewareService<S>;
    type Future = std::future::Ready<Result<Self::Transform, Self::InitError>>;

    // Usamos la sintaxis completamente calificada para evitar ambigüedad
    fn new_transform(&self, service: S) -> <Self as Transform<S, ServiceRequest>>::Future {
        std::future::ready(Ok(ApiKeyMiddlewareService { service }))
    }
}

pub struct ApiKeyMiddlewareService<S> {
    service: S,
}

impl<S> Service<ServiceRequest> for ApiKeyMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse, Error = Error> + 'static,
{
    type Response = ServiceResponse;
    type Error = Error;
    type Future =
        std::pin::Pin<Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(
        &self,
        ctx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.service.poll_ready(ctx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let api_key = std::env::var("API_KEY").unwrap_or_default();

        if api_key.is_empty() {
            log::warn!("API_KEY no está configurada");
            let response = req.into_response(
                HttpResponse::InternalServerError()
                    .json(json!({ "error": "API key no configurada en el servidor" })),
            );
            return Box::pin(std::future::ready(Ok(response)));
        }

        match req.headers().get("X-API-Key") {
            Some(key) if key.to_str().unwrap_or_default() == api_key => {
                let fut = self.service.call(req);
                Box::pin(async move {
                    let res = fut.await?;
                    Ok(res)
                })
            }
            _ => {
                let response = req.into_response(
                    HttpResponse::Unauthorized()
                        .json(json!({ "error": "API key inválida o faltante" })),
                );
                Box::pin(std::future::ready(Ok(response)))
            }
        }
    }
}

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
        .expect("No se pudo conectar a la base de datos SQLite.");

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

    // NUEVO: channel service
    let channel_service = NotificationChannelService::new(db_pool.clone());

    // NotificationService
    let notification_service = NotificationService::new(
        db_pool.clone(),
        email_service.clone(),
        pdf_service.clone(),
        operation_service.clone(),
        channel_service.clone(),
    );

    // Levantar servidor
    log::info!("Levantando servidor en 0.0.0.0:5022");
    HttpServer::new(move || {
        App::new()
            .wrap(ApiKeyMiddleware)
            // Configurar límite de payload a 100MB (104857600 bytes)
            .app_data(web::JsonConfig::default().limit(204857600))
            .app_data(web::Data::new(pdf_service.clone()))
            .app_data(web::Data::new(operation_service.clone()))
            .app_data(web::Data::new(email_service.clone()))
            .app_data(web::Data::new(channel_service.clone()))
            .app_data(web::Data::new(notification_service.clone()))
            .configure(app::init_app)
    })
    .workers(1)
    .bind(("0.0.0.0", 5022))?
    .run()
    .await
}
