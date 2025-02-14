use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use sqlx::{Pool, Sqlite};
use std::env;

use crate::{
    models::{
        email_model::EmailAttachment, notification_model::NotificationRequest,
        pdf_model::PdfRequest,
    },
    services::{
        email_service::EmailService, notification_channel_service::NotificationChannelService,
        operation_service::OperationService, pdf_service::PdfService,
    },
};

#[derive(Clone)]
pub struct NotificationService {
    db_pool: Pool<Sqlite>,
    email_service: EmailService,
    pdf_service: PdfService,
    operation_service: OperationService,
    channel_service: NotificationChannelService,
    http_client: Client,
}

impl NotificationService {
    pub fn new(
        db_pool: Pool<Sqlite>,
        email_service: EmailService,
        pdf_service: PdfService,
        operation_service: OperationService,
        channel_service: NotificationChannelService,
    ) -> Self {
        Self {
            db_pool,
            email_service,
            pdf_service,
            operation_service,
            channel_service,
            http_client: Client::new(),
        }
    }

    /// Procesa la notificación (Email, WhatsApp, etc.).
    pub async fn process_notification(
        &self,
        op_id: String,
        mut req: NotificationRequest,
    ) -> Result<()> {
        log::info!(
            "(process_notification) Iniciando notificación op_id={}...",
            op_id
        );

        // 1) operation -> running
        self.operation_service
            .update_operation_status(&op_id, "running", None)
            .await?;

        // 2) Generar PDF (si pdf_html existe)
        let mut final_attachments = vec![];
        if let Some(html) = req.pdf_html.take() {
            log::info!(
                "(process_notification) Se recibió pdf_html con longitud={} chars, generando PDF...",
                html.len()
            );
            let pdf_bytes = self.generate_pdf_for_notification(&req, html).await?;
            log::info!(
                "(process_notification) PDF generado correctamente con tamaño en bytes: {}",
                pdf_bytes.len()
            );
            let pdf_filename = req
                .pdf_attachment_name
                .clone()
                .unwrap_or_else(|| "document.pdf".to_string());
            final_attachments.push(EmailAttachment {
                filename: pdf_filename.clone(),
                content_type: "application/pdf".to_string(),
                data: pdf_bytes,
            });
            log::info!(
                "(process_notification) Se agregó un adjunto PDF: {}",
                pdf_filename
            );
        } else {
            log::info!("(process_notification) No se recibió pdf_html, no se generará PDF.");
        }

        // 2.b) Agregar otros adjuntos
        if let Some(others) = &req.other_attachments {
            log::info!(
                "(process_notification) Se recibieron {} otros adjuntos.",
                others.len()
            );
            final_attachments.extend(others.clone());
        } else {
            log::info!("(process_notification) No hay 'other_attachments'.");
        }

        // 3) Crear operation_channels para cada canal
        let mut channel_ids = vec![];
        for ch in &req.channels {
            let ch_id = self
                .channel_service
                .create_channel(&op_id, ch, "pending")
                .await?;
            log::info!(
                "(process_notification) Canal '{}' creado en operation_channels con ID={}",
                ch,
                ch_id
            );
            channel_ids.push((ch.clone(), ch_id));
        }

        // 4) Procesar cada canal
        for (channel_name, channel_id) in channel_ids {
            log::info!(
                "(process_notification) Procesando canal '{}' (ID={})...",
                channel_name,
                channel_id
            );

            // Poner en running
            self.channel_service
                .update_channel_status(&channel_id, "running", None, false)
                .await?;

            let result = match channel_name.as_str() {
                "email" => {
                    log::info!("(process_notification) -> Enviando por EMAIL...");
                    self.send_via_email(&op_id, &req, &final_attachments).await
                }
                "whatsapp" => {
                    log::info!("(process_notification) -> Enviando por WHATSAPP...");
                    self.send_via_whatsapp(&op_id, &req, &final_attachments)
                        .await
                }
                other => {
                    let msg = format!("Canal no soportado: {}", other);
                    log::error!("(process_notification) {}", msg);
                    Err(anyhow!(msg))
                }
            };

            match result {
                Ok(_) => {
                    log::info!(
                        "(process_notification) Canal '{}' (ID={}) enviado con éxito. Marcando 'done'...",
                        channel_name,
                        channel_id
                    );
                    self.channel_service
                        .update_channel_status(&channel_id, "done", None, false)
                        .await?;
                }
                Err(e) => {
                    log::error!(
                        "(process_notification) Error al enviar canal '{}' (ID={}): {:?}",
                        channel_name,
                        channel_id,
                        e
                    );
                    // failed
                    self.channel_service
                        .update_channel_status(
                            &channel_id,
                            "failed",
                            Some(&format!("{:?}", e)),
                            true,
                        )
                        .await?;

                    // Marca operation en 'failed'
                    self.operation_service
                        .update_operation_status(&op_id, "failed", Some(&format!("{:?}", e)))
                        .await?;
                }
            }
        }

        // 5) Verificar si todos los canales quedaron en 'done'
        let channels = self
            .channel_service
            .list_channels_for_operation(&op_id)
            .await?;
        let all_done = channels.iter().all(|ch| ch.status == "done");
        if all_done {
            log::info!(
                "(process_notification) Todos los canales para op_id={} están done. Marcando operación 'done'.",
                op_id
            );
            self.operation_service
                .update_operation_status(&op_id, "done", None)
                .await?;
        } else {
            log::info!(
                "(process_notification) No todos los canales están done para op_id={}; la operación podría quedar 'failed' o 'partial'.",
                op_id
            );
        }

        log::info!(
            "(process_notification) Finalizado el proceso para op_id={}.",
            op_id
        );
        Ok(())
    }

    async fn generate_pdf_for_notification(
        &self,
        req: &NotificationRequest,
        html: String,
    ) -> Result<Vec<u8>> {
        log::info!(
            "(generate_pdf_for_notification) Iniciando generación PDF. orientation={:?}, page_size={:?}",
            req.pdf_orientation,
            req.pdf_page_size_preset
        );
        let pdf_req = PdfRequest {
            file_name: req
                .pdf_attachment_name
                .clone()
                .unwrap_or_else(|| "document.pdf".to_string()),
            html,
            orientation: req.pdf_orientation.clone(),
            page_size_preset: req.pdf_page_size_preset.clone(),
            custom_page_size: req.pdf_custom_page_size.clone(),
            margins: req.pdf_margins.clone(),
            scale: req.pdf_scale,
            store_local_pdf: Some(false),
        };

        let pdf_bytes = self
            .pdf_service
            .generate_pdf(pdf_req)
            .await
            .context("Error generando PDF con pdf_service")?;

        log::info!(
            "(generate_pdf_for_notification) PDF generado con éxito ({} bytes).",
            pdf_bytes.len()
        );
        Ok(pdf_bytes)
    }

    async fn send_via_email(
        &self,
        op_id: &str,
        req: &NotificationRequest,
        attachments: &[EmailAttachment],
    ) -> Result<()> {
        log::info!(
            "(send_via_email) Preparando envío de correo para op_id={}. Adjuntos={}",
            op_id,
            attachments.len()
        );
        let email_config = req
            .email_config
            .as_ref()
            .ok_or_else(|| anyhow!("Falta email_config para canal email"))?;

        log::info!(
            "(send_via_email) SMTP host={}, user={}, recipients={:?}",
            email_config.smtp_host,
            email_config.smtp_user,
            email_config.recipients
        );

        // Armamos la request para el EmailService
        let send_req = crate::models::email_model::SendUniversalEmailRequest {
            smtp_host: email_config.smtp_host.clone(),
            smtp_port: email_config.smtp_port,
            smtp_user: email_config.smtp_user.clone(),
            smtp_pass: email_config.smtp_pass.clone(),
            recipients: email_config.recipients.clone(),
            subject: req.subject.clone().unwrap_or_default(),
            body: req.body.clone().unwrap_or_default(),
            async_send: false,
            // No generaremos PDF acá, pues ya lo hicimos arriba en `final_attachments`.
            pdf_html: None,
            pdf_orientation: None,
            pdf_page_size_preset: None,
            pdf_custom_page_size: None,
            pdf_margins: None,
            pdf_scale: None,
            pdf_attachment_name: None,
            // Los adjuntos van tanto aquí (para referencia) como en el tercer param
            other_attachments: Some(attachments.to_vec()),
        };

        log::info!("(send_via_email) Llamando a email_service.send_unified...");
        self.email_service
            .send_unified(op_id.to_string(), send_req, attachments.to_vec())
            .await
            .context("(send_via_email) Fallo en email_service")?;

        log::info!(
            "(send_via_email) -> Correo enviado con éxito para op_id={}.",
            op_id
        );
        Ok(())
    }

    async fn send_via_whatsapp(
        &self,
        op_id: &str,
        req: &NotificationRequest,
        attachments: &[EmailAttachment],
    ) -> Result<()> {
        log::info!(
            "(send_via_whatsapp) Iniciando envío por WhatsApp para op_id={}. Adjuntos={}",
            op_id,
            attachments.len()
        );

        // Variables de entorno
        let base_url =
            env::var("WHATSAPP_API_URL").map_err(|_| anyhow!("No se definió WHATSAPP_API_URL"))?;
        let session_id = env::var("WHATSAPP_API_SESSION_ID")
            .map_err(|_| anyhow!("No se definió WHATSAPP_API_SESSION_ID"))?;

        log::info!(
            "(send_via_whatsapp) base_url={}, session_id={}",
            base_url,
            session_id
        );

        // Extraer recipients de la config
        let wa_config = req
            .whatsapp_config
            .as_ref()
            .ok_or_else(|| anyhow!("No se proporcionó whatsapp_config"))?;
        let recipients = &wa_config.recipients;
        let message = wa_config.message.clone().unwrap_or_default();
        log::info!("(send_via_whatsapp) recipients={:?}", recipients);

        // 1) Revisar si la sesión está conectada
        let status_url = format!("{}/session/status/{}", base_url, session_id);
        log::info!(
            "(send_via_whatsapp) Consultando status en URL={}",
            status_url
        );
        let resp = self
            .http_client
            .get(&status_url)
            .send()
            .await
            .context("Fallo al hacer GET session/status")?;

        log::info!(
            "(send_via_whatsapp) Status code de la respuesta={}",
            resp.status()
        );
        if !resp.status().is_success() {
            let body_txt = resp.text().await.unwrap_or_default();
            log::error!(
                "(send_via_whatsapp) La respuesta NO es exitosa. body_txt='{}'",
                body_txt
            );
            return Err(anyhow!("Error consultando sesión: {}", body_txt));
        }
        let json_val = resp.json::<serde_json::Value>().await?;
        log::info!("(send_via_whatsapp) Respuesta JSON status={:?}", json_val);

        let connected = json_val
            .get("state")
            .and_then(|v| v.as_str())
            .map(|s| s == "CONNECTED")
            .unwrap_or(false);

        if !connected {
            log::error!("(send_via_whatsapp) Sesión WhatsApp no está CONNECTED.");
            return Err(anyhow!("Sesión WhatsApp no está CONNECTED"));
        }

        // 2) Enviar mensaje de texto
        if message.len() > 0 {
            log::info!(
                "(send_via_whatsapp) Enviando texto a {} destinatarios...",
                recipients.len()
            );
            for chat_id in recipients {
                log::info!(
                    "(send_via_whatsapp) -> chat_id='{}', body='{}'",
                    chat_id,
                    message
                );
                let send_url = format!("{}/client/sendMessage/{}", base_url, session_id);
                let payload = serde_json::json!({
                    "chatId": chat_id,
                    "contentType": "string",
                    "content": message
                });

                let r = self
                    .http_client
                    .post(&send_url)
                    .json(&payload)
                    .send()
                    .await
                    .context("(send_via_whatsapp) Fallo al POST para texto")?;

                log::info!(
                    "(send_via_whatsapp) -> Envío texto a '{}': status={}",
                    chat_id,
                    r.status()
                );
                if !r.status().is_success() {
                    let e = r.text().await.unwrap_or_default();
                    log::error!(
                        "(send_via_whatsapp) -> Fallo al enviar texto a '{}': {}",
                        chat_id,
                        e
                    );
                    return Err(anyhow!("Fallo al enviar texto WhatsApp: {}", e));
                }
            }
        } else {
            log::info!("(send_via_whatsapp) No hay body_txt, no se envía mensaje de texto.");
        }

        // 3) Enviar adjuntos
        if !attachments.is_empty() {
            log::info!(
                "(send_via_whatsapp) Enviando {} adjuntos...",
                attachments.len()
            );
        } else {
            log::info!("(send_via_whatsapp) Sin adjuntos, nada que enviar.");
        }

        for attach in attachments {
            let base64_data = base64::encode(&attach.data);
            log::info!(
                "(send_via_whatsapp) -> Adjunto '{}', mimetype='{}', data_len={}",
                attach.filename,
                attach.content_type,
                attach.data.len()
            );
            for chat_id in recipients {
                let send_url = format!("{}/client/sendMessage/{}", base_url, session_id);
                let payload = serde_json::json!({
                    "chatId": chat_id,
                    "contentType": "MessageMedia",
                    "content": {
                        "mimetype": attach.content_type,
                        "data": base64_data,
                        "filename": attach.filename
                    }
                });

                let r = self
                    .http_client
                    .post(&send_url)
                    .json(&payload)
                    .send()
                    .await
                    .context("(send_via_whatsapp) Fallo al POST para adjunto")?;

                log::info!(
                    "(send_via_whatsapp) -> Envío adjunto a '{}': status={}",
                    chat_id,
                    r.status()
                );

                if !r.status().is_success() {
                    let e = r.text().await.unwrap_or_default();
                    log::error!(
                        "(send_via_whatsapp) -> Fallo al enviar adjunto a '{}': {}",
                        chat_id,
                        e
                    );
                    return Err(anyhow!("Fallo al enviar adjunto WhatsApp: {}", e));
                }
            }
        }

        log::info!(
            "(send_via_whatsapp) Finalizado envío WhatsApp para op_id={}.",
            op_id
        );
        Ok(())
    }

    // pub async fn reattempt_failed_channels(&self) -> Result<()> {
    //     let max_retries = 5;

    //     log::info!(
    //         "(reattempt_failed_channels) Iniciando reintentos para attempts < {}",
    //         max_retries
    //     );

    //     // Buscar operation_channels en failed/pending con attempts < max
    //     let rows = sqlx::query!(
    //         r#"
    //         SELECT id, operation_id, channel, attempts
    //         FROM operation_channels
    //         WHERE (status='failed' OR status='pending')
    //           AND attempts < ?1
    //         "#,
    //         max_retries
    //     )
    //     .fetch_all(&self.db_pool)
    //     .await?;

    //     log::info!(
    //         "(reattempt_failed_channels) Se encontraron {} canales para reintentar.",
    //         rows.len()
    //     );

    //     for row in rows {
    //         let ch_id = match row.id {
    //             Some(val) => val,
    //             None => {
    //                 log::error!("(reattempt_failed_channels) Canal sin ID, ignorando...");
    //                 continue;
    //             }
    //         };
    //         let op_id = row.operation_id.clone();
    //         let channel_name = row.channel.clone();
    //         let attempts = row.attempts;

    //         log::info!(
    //             "(reattempt_failed_channels) Reintentando canal='{}', op_id='{}', attempts={}",
    //             channel_name,
    //             op_id,
    //             attempts
    //         );

    //         // Poner en running, attempts++
    //         self.channel_service
    //             .update_channel_status(&ch_id, "running", None, true)
    //             .await?;

    //         // En un proyecto real, necesitas recargar la info original
    //         // (NotificationRequest). Aquí se simplifica.
    //         let result = if channel_name == "whatsapp" {
    //             log::info!(
    //                 "(reattempt_failed_channels) Reintentando WhatsApp con request dummy..."
    //             );
    //             let dummy_req = NotificationRequest {
    //                 channels: vec!["whatsapp".to_string()],
    //                 email_config: None,
    //                 whatsapp_config: Some(WhatsAppConfig {
    //                     recipients: vec!["123456789@c.us".to_string()],

    //                 }),
    //                 subject: Some("Reintento".to_string()),
    //                 body: Some("Mensaje reintentado".to_string()),
    //                 async_send: false,
    //                 pdf_html: None,
    //                 pdf_orientation: None,
    //                 pdf_page_size_preset: None,
    //                 pdf_custom_page_size: None,
    //                 pdf_margins: None,
    //                 pdf_scale: None,
    //                 pdf_attachment_name: None,
    //                 other_attachments: None,
    //             };
    //             // Sin adjuntos
    //             self.send_via_whatsapp(&op_id, &dummy_req, &[]).await
    //         } else if channel_name == "email" {
    //             log::info!("(reattempt_failed_channels) Reintento de email no implementado.");
    //             Err(anyhow!("Reintento de email no implementado"))
    //         } else {
    //             let msg = format!("Canal desconocido: {}", channel_name);
    //             log::error!("(reattempt_failed_channels) {}", msg);
    //             Err(anyhow!(msg))
    //         };

    //         match result {
    //             Ok(_) => {
    //                 log::info!(
    //                     "(reattempt_failed_channels) Reintento canal='{}' -> success. Marcando done.",
    //                     channel_name
    //                 );
    //                 self.channel_service
    //                     .update_channel_status(&ch_id, "done", None, false)
    //                     .await?;
    //             }
    //             Err(e) => {
    //                 log::error!(
    //                     "(reattempt_failed_channels) Reintento canal='{}' -> error: {:?}",
    //                     channel_name,
    //                     e
    //                 );
    //                 self.channel_service
    //                     .update_channel_status(&ch_id, "failed", Some(&format!("{:?}", e)), false)
    //                     .await?;
    //             }
    //         }

    //         // Revisa si todos los canales quedaron done => op done
    //         let channels = self
    //             .channel_service
    //             .list_channels_for_operation(&op_id)
    //             .await?;
    //         let all_done = channels.iter().all(|ch| ch.status == "done");
    //         if all_done {
    //             log::info!(
    //                 "(reattempt_failed_channels) Todos los canales 'done' para op_id='{}'. Marcando operation done.",
    //                 op_id
    //             );
    //             self.operation_service
    //                 .update_operation_status(&op_id, "done", None)
    //                 .await?;
    //         }
    //     }

    //     Ok(())
    // }
}
