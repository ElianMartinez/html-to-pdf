//! tests/pdf_tests.rs
//! Pruebas unitarias para `PdfService`.

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::Instant;

    use crate::config::pdf_config::PdfGlobalConfig;
    use crate::models::pdf_model::{PaperSize, PdfMargins, PdfRequest};
    use crate::services::pdf_service::PdfService;
    use actix_rt::test;
    use anyhow::Result;
    use chromiumoxide::browser::{Browser, BrowserConfig};
    use futures_util::StreamExt;
    use uuid::Uuid; // para tests async con tokio+actix

    // Helper: crea un BrowserConfig único, usando user-data-dir en /tmp
    fn create_unique_chrome_config() -> BrowserConfig {
        let unique_id = Uuid::new_v4().to_string();
        let tmp_dir = format!("/tmp/chrome-test-{}", unique_id);

        // Crear el directorio
        fs::create_dir_all(&tmp_dir).expect("Failed to create temp dir");

        // Podría causar conflictos si varios tests usan la misma ruta
        // o si Chrome se niega a arrancar varios perfiles en paralelo.
        // Forzar test secuencial con cargo test -- --test-threads=1
        BrowserConfig::builder()
            .args(vec![
                "--headless",
                "--no-sandbox",
                "--disable-setuid-sandbox",
                "--disable-gpu",
                "--disable-dev-shm-usage",
                "--no-first-run",
                "--no-default-browser-check",
                "--disable-background-networking",
                "--disable-background-timer-throttling",
                "--disable-backgrounding-occluded-windows",
                "--disable-breakpad",
                "--disable-client-side-phishing-detection",
                "--disable-default-apps",
                "--disable-extensions",
                "--disable-popup-blocking",
                "--disable-sync",
                "--disable-translate",
                &format!("--user-data-dir={}", tmp_dir),
            ])
            .build()
            .expect("Failed to build BrowserConfig")
    }

    // Helper: crea un PdfService con config de Chrome única.
    async fn create_test_service() -> Result<PdfService> {
        let pdf_config = PdfGlobalConfig::default();
        let browser_config = create_unique_chrome_config();

        // Lanza el browser
        let (browser, mut handler) = Browser::launch(browser_config).await?;

        // Manejamos eventos en un task
        let handle = tokio::spawn(async move {
            while let Some(_evt) = handler.next().await {
                // debug, logs, etc.
            }
        });

        // Usa el constructor de test
        Ok(PdfService::new_test(browser, handle, pdf_config).await)
    }

    #[test]
    async fn test_generate_pdf_simple() {
        let service = create_test_service()
            .await
            .expect("Failed to create service");

        let request = PdfRequest {
            html: "<h1>Hola mundo</h1>".to_string(),
            orientation: Some("portrait".to_string()),
            paper_size: None,
            margins: None,
            size_category: None,
        };

        let result = service.generate_pdf(request).await;
        assert!(result.is_ok());
        let pdf_data = result.unwrap();
        assert!(pdf_data.starts_with(b"%PDF"), "No inicia con %PDF");
    }

    #[test]
    async fn test_generate_pdf_large() {
        let service = create_test_service()
            .await
            .expect("Failed to create service");

        let big_html = "<h1>Linea</h1>".repeat(5000);
        let req = PdfRequest {
            html: big_html,
            orientation: Some("landscape".to_string()),
            paper_size: Some(PaperSize {
                width: 11.0,
                height: 8.5,
            }),
            margins: Some(PdfMargins {
                top: 1.0,
                bottom: 1.0,
                left: 1.0,
                right: 1.0,
            }),
            size_category: Some("large".to_string()),
        };

        let res = service.generate_pdf(req).await;
        assert!(res.is_ok(), "No se generó PDF (large)");
        let pdf_data = res.unwrap();
        assert!(pdf_data.starts_with(b"%PDF"));
    }

    #[test]
    async fn test_memory_usage_multiple_pdfs() {
        let service = create_test_service()
            .await
            .expect("Failed to create service");

        for i in 0..3 {
            let req = PdfRequest {
                html: format!("<h1>Iter {}</h1>", i),
                orientation: Some("portrait".to_string()),
                paper_size: None,
                margins: None,
                size_category: None,
            };

            let res = service.generate_pdf(req).await;
            assert!(res.is_ok());
            let pdf_data = res.unwrap();
            assert!(pdf_data.starts_with(b"%PDF"));
        }
    }

    #[test]
    async fn test_pdf_orientations() {
        let service = create_test_service()
            .await
            .expect("Failed to create service");

        for orient in &["portrait", "landscape"] {
            let req = PdfRequest {
                html: format!("<h1>Orient {}</h1>", orient),
                orientation: Some(orient.to_string()),
                paper_size: None,
                margins: None,
                size_category: None,
            };

            let res = service.generate_pdf(req).await;
            assert!(res.is_ok(), "Falló con orientación {}", orient);
            assert!(res.unwrap().starts_with(b"%PDF"));
        }
    }

    #[test]
    async fn test_temp_file_cleanup() {
        let service = create_test_service()
            .await
            .expect("Failed to create service");

        let tmp_dir = PathBuf::from("./tmp");
        let initial_count = fs::read_dir(&tmp_dir).map(|d| d.count()).unwrap_or(0);

        let req = PdfRequest {
            html: "<h1>Temp file test</h1>".repeat(1000),
            orientation: None,
            paper_size: None,
            margins: None,
            size_category: Some("large".to_string()),
        };

        let res = service.generate_pdf(req).await;
        assert!(res.is_ok());

        let final_count = fs::read_dir(&tmp_dir).map(|d| d.count()).unwrap_or(0);
        assert_eq!(initial_count, final_count, "Quedaron archivos en ./tmp");
    }

    #[test]
    async fn test_pdf_performance() {
        let service = create_test_service()
            .await
            .expect("Failed to create service");

        let start = Instant::now();
        let req = PdfRequest {
            html: "<h1>Performance Test</h1>".to_string(),
            orientation: Some("portrait".to_string()),
            paper_size: None,
            margins: None,
            size_category: None,
        };

        let res = service.generate_pdf(req).await;
        assert!(res.is_ok());

        let duration = start.elapsed();
        assert!(duration.as_secs() < 10, "Tardó demasiado: {:?}", duration);
    }

    #[test]
    async fn test_different_paper_sizes() {
        let service = create_test_service()
            .await
            .expect("Failed to create service");

        let sizes = vec![
            (8.5, 11.0),   // Letter
            (11.0, 17.0),  // Tabloid
            (8.27, 11.69), // A4
        ];

        for (w, h) in sizes {
            let req = PdfRequest {
                html: format!("<h1>Size {}x{}</h1>", w, h),
                orientation: None,
                paper_size: Some(PaperSize {
                    width: w,
                    height: h,
                }),
                margins: None,
                size_category: None,
            };

            let res = service.generate_pdf(req).await;
            assert!(res.is_ok(), "Falló con tamaño {}x{}", w, h);
            assert!(res.unwrap().starts_with(b"%PDF"));
        }
    }
}
