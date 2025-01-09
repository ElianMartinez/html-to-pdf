//! services/email_service.rs
use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use lettre::{
    transport::smtp::{authentication::Credentials, client::Tls, client::TlsParameters},
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};
use sqlx::{Pool, Sqlite};
use uuid::Uuid;

use crate::models::email_model::{EmailStatusResponse, SendEmailRequest};
use crate::models::operation_model::{CreateOperationRequest, CreateOperationResponse};
use crate::services::operation_service::OperationService;

/// EmailService depende de OperationService para crear/actualizar operaciones.
#[derive(Clone)]
pub struct EmailService {
    db_pool: Pool<Sqlite>,
    op_service: OperationService,
}

impl EmailService {
    pub fn new(db_pool: Pool<Sqlite>, op_service: OperationService) -> Self {
        EmailService {
            db_pool,
            op_service,
        }
    }

    pub async fn run_migrations(&self) -> Result<()> {
        sqlx::migrate!("./migrations").run(&self.db_pool).await?;
        Ok(())
    }

    /// Envía (o agenda) un email. Devuelve el `operation_id`.
    pub async fn send_email(&self, req: SendEmailRequest) -> Result<String> {
        // 1) Crear Operación (status = 'pending') en tabla `operations`
        let meta_json = Some(format!(
            r#"{{"smtp_host":"{}","smtp_port":{},"to":"{}","subject":"{}"}}"#,
            req.smtp_host, req.smtp_port, req.recipient, req.subject
        ));

        let create_req = CreateOperationRequest {
            operation_type: "send_email".to_string(),
            is_async: req.async_send,
            metadata: meta_json,
        };
        let create_resp: CreateOperationResponse = self
            .op_service
            .create_operation(create_req)
            .await
            .context("No se pudo crear operación 'send_email'")?;

        // El ID de la operación
        let operation_id = create_resp.id.clone();

        // 2) Insertar en "emails"
        let now = Utc::now().to_rfc3339();

        let email_id = Uuid::new_v4().to_string();
        sqlx::query!(
            r#"
            INSERT INTO emails (
                id,
                operation_id,
                recipient,
                subject,
                body,
                status,
                created_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, 'pending', ?6)
            "#,
            email_id,
            operation_id,
            req.recipient,
            req.subject,
            req.body,
            now
        )
        .execute(&self.db_pool)
        .await
        .context("Fallo al insertar registro en 'emails'")?;

        // 3) Si async => spawn. Si sync => inline
        if req.async_send {
            let pool_clone = self.db_pool.clone();
            let op_svc_clone = self.op_service.clone();
            let operation_id_clone = operation_id.clone();
            tokio::spawn(async move {
                if let Err(e) = process_email_in_background(
                    pool_clone,
                    op_svc_clone,
                    operation_id_clone.clone(),
                    req,
                )
                .await
                {
                    log::error!(
                        "Error en envío asíncrono de email {}: {:?}",
                        operation_id_clone,
                        e
                    );
                }
            });
            // Retornamos el id de la operación, sin bloquear
            Ok(operation_id)
        } else {
            // Modo síncrono:
            // Actualizamos operation -> "running"
            self.op_service
                .update_operation(&operation_id, "running", None)
                .await
                .ok();

            // Enviamos
            match send_email_smtp(&req).await {
                Ok(_) => {
                    // operation -> "done", emails -> "sent"
                    self.op_service
                        .update_operation(&operation_id, "done", None)
                        .await
                        .ok();

                    sqlx::query!(
                        r#"UPDATE emails SET status = 'sent' WHERE operation_id = ?1"#,
                        operation_id
                    )
                    .execute(&self.db_pool)
                    .await
                    .ok();
                    Ok(operation_id)
                }
                Err(e) => {
                    let msg = format!("{:?}", e);
                    // operation -> "failed"
                    self.op_service
                        .update_operation(&operation_id, "failed", Some(&msg))
                        .await
                        .ok();

                    sqlx::query!(
                        r#"
                        UPDATE emails
                        SET status = 'failed', error_message = ?1
                        WHERE operation_id = ?2
                        "#,
                        msg,
                        operation_id
                    )
                    .execute(&self.db_pool)
                    .await
                    .ok();

                    Err(anyhow!("Fallo al enviar email síncrono: {}", msg))
                }
            }
        }
    }

    /// Consulta el estado del email por operation_id.
    pub async fn get_email_status(&self, operation_id: &str) -> Result<EmailStatusResponse> {
        let row = sqlx::query!(
            r#"
            SELECT status, error_message
            FROM emails
            WHERE operation_id = ?1
            LIMIT 1
            "#,
            operation_id
        )
        .fetch_one(&self.db_pool)
        .await
        .context("No se encontró email con esa operación")?;

        Ok(EmailStatusResponse {
            id: operation_id.to_string(),
            status: row.status,
            error: row.error_message,
        })
    }
}

/// Corre en background (spawn) si async_send = true
async fn process_email_in_background(
    db_pool: Pool<Sqlite>,
    op_service: OperationService,
    operation_id: String,
    req: SendEmailRequest,
) -> Result<()> {
    // operation -> "running"
    op_service
        .update_operation(&operation_id, "running", None)
        .await
        .ok();

    // enviar
    match send_email_smtp(&req).await {
        Ok(_) => {
            // operation -> "done", email -> "sent"
            op_service
                .update_operation(&operation_id, "done", None)
                .await
                .ok();

            sqlx::query!(
                r#"UPDATE emails SET status = 'sent' WHERE operation_id = ?1"#,
                operation_id
            )
            .execute(&db_pool)
            .await
            .ok();

            Ok(())
        }
        Err(e) => {
            let msg = format!("{:?}", e);
            // operation -> "failed"
            op_service
                .update_operation(&operation_id, "failed", Some(&msg))
                .await
                .ok();

            sqlx::query!(
                r#"
                UPDATE emails
                SET status = 'failed', error_message = ?1
                WHERE operation_id = ?2
                "#,
                msg,
                operation_id
            )
            .execute(&db_pool)
            .await
            .ok();

            Err(anyhow!("Fallo al enviar email (background): {}", msg))
        }
    }
}

/// Ejemplo de envío real con `lettre`
async fn send_email_smtp(req: &SendEmailRequest) -> Result<()> {
    let email = Message::builder()
        .from(req.smtp_user.parse()?)
        .to(req.recipient.parse()?)
        .subject(&req.subject)
        .body(req.body.clone())?;

    // Configurar los parámetros TLS
    let tls_parameters = TlsParameters::new(req.smtp_host.clone())
        .map_err(|e| anyhow!("Error configurando TLS: {:?}", e))?;

    // Configurar el transporte SMTP con TLS
    let mailer = AsyncSmtpTransport::<Tokio1Executor>::relay(&req.smtp_host)?
        .port(req.smtp_port)
        .credentials(Credentials::new(
            req.smtp_user.clone(),
            req.smtp_pass.clone(),
        ))
        .tls(Tls::Required(tls_parameters))
        .build();

    // Enviar el email
    mailer
        .send(email)
        .await
        .map_err(|e| anyhow!("Error en envío SMTP: {:?}", e))?;

    Ok(())
}
