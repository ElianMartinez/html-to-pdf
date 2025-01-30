//! services/email_service.rs

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use lettre::{
    message::{
        header::{ContentDisposition, ContentType},
        Body, Mailbox, MultiPart, SinglePart,
    },
    transport::smtp::{
        authentication::Credentials,
        client::{Tls, TlsParameters},
    },
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};

use sqlx::{Pool, Sqlite};

use crate::{
    models::email_model::{EmailAttachment, EmailStatusResponse, SendUniversalEmailRequest},
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

    /// Ejecuta migraciones de la base de datos (emails)
    pub async fn run_migrations(&self) -> Result<()> {
        sqlx::migrate!("./migrations")
            .run(&self.db_pool)
            .await
            .context("Failed to run email service migrations")?;
        Ok(())
    }

    // ======================================================
    // 2) NUEVO método unificado: múltiples recipients, adjuntos
    // ======================================================
    pub async fn send_unified(
        &self,
        op_id: String,
        req: SendUniversalEmailRequest,
        attachments: Vec<EmailAttachment>,
    ) -> Result<String> {
        // Insertar en tabla emails con recipients unificados
        self.insert_email_record_multiple(&op_id, &req).await?;

        if req.async_send {
            // Spawn asíncrono
            self.spawn_async_email_task_multiple(op_id.clone(), req, attachments)
                .await?;
            Ok(op_id)
        } else {
            self.handle_sync_email_multiple(&op_id, req, attachments)
                .await
        }
    }

    // ----------------------------------------------------------------
    // Consultar el estado del email por operation_id
    // ----------------------------------------------------------------
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

    // ========================================================================
    // Métodos privados para la versión "multiple" (NUEVA)
    // ========================================================================

    async fn insert_email_record_multiple(
        &self,
        operation_id: &str,
        req: &SendUniversalEmailRequest,
    ) -> Result<()> {
        let email_id = uuid::Uuid::new_v4().to_string();
        let created_at = Utc::now().to_rfc3339();

        // Unificamos los recipients en un string:
        let joined_recipients = req.recipients.join(";");
        let subject = &req.subject;
        let body = &req.body;

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
            joined_recipients,
            subject,
            body,
            created_at
        )
        .execute(&self.db_pool)
        .await
        .context("Failed to insert email record (multiple)")?;

        Ok(())
    }

    async fn handle_sync_email_multiple(
        &self,
        operation_id: &str,
        req: SendUniversalEmailRequest,
        attachments: Vec<EmailAttachment>,
    ) -> Result<String> {
        self.update_operation_status(operation_id, "running", None)
            .await?;

        match self.send_email_via_smtp_multiple(&req, attachments).await {
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

    async fn send_email_via_smtp_multiple(
        &self,
        req: &SendUniversalEmailRequest,
        attachments: Vec<EmailAttachment>,
    ) -> Result<()> {
        let from: Mailbox = format!("Calipso Dynamics <{}>", req.smtp_user)
            .parse()
            .context("Invalid from address")?;

        let tls_params = TlsParameters::new(req.smtp_host.clone())?;
        let mailer = AsyncSmtpTransport::<Tokio1Executor>::relay(&req.smtp_host)?
            .port(req.smtp_port)
            .credentials(Credentials::new(
                req.smtp_user.clone(),
                req.smtp_pass.clone(),
            ))
            .tls(Tls::Required(tls_params))
            .build();

        // Construir cuerpo en HTML
        let html_part = SinglePart::builder()
            .header(ContentType::parse("text/html; charset=utf-8")?)
            .body(req.body.clone());

        let mut multipart = MultiPart::mixed().singlepart(html_part);

        for attach in attachments {
            let body = Body::new(attach.data);
            let part = SinglePart::builder()
                .header(ContentType::parse(attach.content_type.as_str())?)
                .header(ContentDisposition::attachment(&attach.filename.clone()))
                .body(body);
            multipart = multipart.singlepart(part);
        }

        // Enviar uno por uno a la lista
        for recip_str in &req.recipients {
            let to: Mailbox = recip_str.parse().context("Invalid recipient address")?;
            let builder = Message::builder()
                .from(from.clone())
                .to(to)
                .subject(&req.subject);

            let message = builder.multipart(multipart.clone())?;

            tokio::time::timeout(std::time::Duration::from_secs(30), mailer.send(message))
                .await??;
        }

        Ok(())
    }

    async fn spawn_async_email_task_multiple(
        &self,
        operation_id: String,
        req: SendUniversalEmailRequest,
        attachments: Vec<EmailAttachment>,
    ) -> Result<()> {
        let db_pool = self.db_pool.clone();
        let op_service = self.op_service.clone();

        tokio::spawn(async move {
            let email_service = EmailService::new(db_pool, op_service);
            match email_service
                .handle_sync_email_multiple(&operation_id, req, attachments)
                .await
            {
                Ok(_) => log::info!("Async email {} sent successfully", operation_id),
                Err(e) => log::error!("Failed async email {}: {}", operation_id, e),
            }
        });

        Ok(())
    }

    // ========================================================================
    // Métodos comunes de actualización de estado
    // ========================================================================

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
