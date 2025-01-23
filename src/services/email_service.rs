//! services/email_service.rs
use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use lettre::{
    message::{
        header::{ContentDisposition, ContentType},
        Attachment, Body, Mailbox, MultiPart, SinglePart,
    },
    transport::smtp::{
        authentication::Credentials,
        client::{Tls, TlsParameters},
    },
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};
use serde_json::json;
use sqlx::{Pool, Sqlite};
use uuid::Uuid;

use crate::{
    models::{
        email_model::{EmailAttachment, EmailStatusResponse, SendEmailRequest},
        operation_model::{CreateOperationRequest, CreateOperationResponse},
    },
    services::operation_service::OperationService,
};

#[derive(Debug, Clone)]
pub struct EmailService {
    db_pool: Pool<Sqlite>,
    op_service: OperationService,
}

impl EmailService {
    pub fn new(db_pool: Pool<Sqlite>, op_service: OperationService) -> Self {
        Self {
            db_pool,
            op_service,
        }
    }

    /// Ejecuta migraciones de la base de datos
    pub async fn run_migrations(&self) -> Result<()> {
        sqlx::migrate!("./migrations")
            .run(&self.db_pool)
            .await
            .context("Failed to run email service migrations")?;
        Ok(())
    }

    /// Método principal para enviar emails (con o sin adjuntos)
    pub async fn send_email(&self, req: SendEmailRequest) -> Result<String> {
        let operation = self.create_email_operation(&req).await?;
        self.insert_email_record(&operation.id, &req).await?;

        if req.async_send {
            self.spawn_async_email_task(operation.id.clone(), req)
                .await?;
            Ok(operation.id)
        } else {
            self.handle_sync_email(&operation.id, req).await
        }
    }

    /// Versión con adjuntos (manteniendo retrocompatibilidad)
    pub async fn send_email_with_attachments(
        &self,
        mut req: SendEmailRequest,
        attachments: Vec<EmailAttachment>,
    ) -> Result<String> {
        req.attachments = Some(attachments);
        self.send_email(req).await
    }

    /// Consulta el estado del email por operation_id
    pub async fn get_email_status(&self, operation_id: &str) -> Result<EmailStatusResponse> {
        let record = sqlx::query!(
            r#"SELECT status, error_message FROM emails WHERE operation_id = ?1"#,
            operation_id
        )
        .fetch_one(&self.db_pool)
        .await
        .context("Email operation not found")?;

        Ok(EmailStatusResponse {
            id: operation_id.to_string(),
            status: record.status,
            error: record.error_message,
        })
    }

    // --- Métodos internos ---

    /// Crea la operación en la base de datos
    async fn create_email_operation(
        &self,
        req: &SendEmailRequest,
    ) -> Result<CreateOperationResponse> {
        let metadata = json!({
            "smtp_host": req.smtp_host,
            "smtp_port": req.smtp_port,
            "recipient": req.recipient,
            "subject": req.subject,
            "attachments": req.attachments.as_ref().map(|a| a.len()).unwrap_or(0)
        });

        let create_req = CreateOperationRequest {
            operation_type: "send_email".to_string(),
            is_async: req.async_send,
            metadata: Some(metadata.to_string()),
        };

        self.op_service
            .create_operation(create_req)
            .await
            .context("Failed to create email operation")
    }

    /// Inserta el registro en la tabla emails
    async fn insert_email_record(&self, operation_id: &str, req: &SendEmailRequest) -> Result<()> {
        let email_id = Uuid::new_v4().to_string();
        let created_at = Utc::now().to_rfc3339();
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
            created_at
        )
        .execute(&self.db_pool)
        .await
        .context("Failed to insert email record")?;

        Ok(())
    }

    /// Maneja el envío síncrono
    async fn handle_sync_email(&self, operation_id: &str, req: SendEmailRequest) -> Result<String> {
        self.update_operation_status(operation_id, "running", None)
            .await?;

        match self.send_email_via_smtp(&req).await {
            Ok(_) => {
                self.update_email_status(operation_id, "sent", None).await?;
                Ok(operation_id.to_string())
            }
            Err(e) => {
                let error = format!("{e:?}");
                self.update_email_status(operation_id, "failed", Some(&error))
                    .await?;
                Err(anyhow!("Email send failed: {error}"))
            }
        }
    }

    /// Actualiza el estado en ambas tablas
    async fn update_email_status(
        &self,
        operation_id: &str,
        status: &str,
        error: Option<&str>,
    ) -> Result<()> {
        sqlx::query!(
            r#"UPDATE operations SET status = ?1, error_message = ?2 WHERE id = ?3"#,
            status,
            error,
            operation_id
        )
        .execute(&self.db_pool)
        .await
        .context("Failed to update operation status")?;

        sqlx::query!(
            r#"UPDATE emails SET status = ?1, error_message = ?2 WHERE operation_id = ?3"#,
            status,
            error,
            operation_id
        )
        .execute(&self.db_pool)
        .await
        .context("Failed to update email status")?;

        Ok(())
    }

    /// Envía el email usando SMTP con soporte para adjuntos
    async fn send_email_via_smtp(&self, req: &SendEmailRequest) -> Result<()> {
        let from: Mailbox = format!("PDF Service <{}>", req.smtp_user)
            .parse()
            .context("Invalid from address")?;

        let to: Mailbox = req.recipient.parse().context("Invalid recipient address")?;

        let builder = Message::builder().from(from).to(to).subject(&req.subject);

        // Construir cuerpo del mensaje
        let text_part = SinglePart::plain(req.body.clone());
        let mut multipart = MultiPart::mixed().singlepart(text_part);

        // Añadir adjuntos
        if let Some(attachments) = &req.attachments {
            for attachment in attachments {
                let body = Body::new(attachment.data.clone());
                let part = SinglePart::builder()
                    .header(ContentType::parse(attachment.content_type.as_str())?)
                    .header(ContentDisposition::attachment(&attachment.filename.clone()))
                    .body(body);
                multipart = multipart.singlepart(part);
            }
        }

        let message = builder.multipart(multipart)?;

        // Configuración segura de TLS
        let tls_params = TlsParameters::new(req.smtp_host.clone())?;
        let mailer = AsyncSmtpTransport::<Tokio1Executor>::relay(&req.smtp_host)?
            .port(req.smtp_port)
            .credentials(Credentials::new(
                req.smtp_user.clone(),
                req.smtp_pass.clone(),
            ))
            .tls(Tls::Required(tls_params))
            .build();

        // Envío real con timeout
        tokio::time::timeout(std::time::Duration::from_secs(30), mailer.send(message)).await??;

        Ok(())
    }

    /// Spawn para manejar emails asíncronos
    async fn spawn_async_email_task(
        &self,
        operation_id: String,
        req: SendEmailRequest,
    ) -> Result<()> {
        let db_pool = self.db_pool.clone();
        let op_service = self.op_service.clone();

        tokio::spawn(async move {
            let email_service = EmailService::new(db_pool, op_service);

            match email_service.handle_sync_email(&operation_id, req).await {
                Ok(_) => log::info!("Async email {} sent successfully", operation_id),
                Err(e) => log::error!("Failed async email {}: {}", operation_id, e),
            }
        });

        Ok(())
    }

    /// Actualiza solo el estado de la operación
    async fn update_operation_status(
        &self,
        operation_id: &str,
        status: &str,
        error: Option<&str>,
    ) -> Result<()> {
        self.op_service
            .update_operation_status(operation_id, status, error)
            .await
            .context("Failed to update operation status")
    }
}
