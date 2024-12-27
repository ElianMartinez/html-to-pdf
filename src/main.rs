// html-to-pdf-memory.rs

use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use headless_chrome::{types::PrintToPdfOptions, Browser, LaunchOptionsBuilder};
use serde::Deserialize;
use std::env;

#[derive(Deserialize)]
struct Config {
    chrome_path: String,
    orientation: String,   // "portrait" o "landscape"
    paper_size: PaperSize, // medidas en pulgadas
    margins: Margins,      // márgenes en pulgadas
    html_content: String,  // HTML sin codificar (texto normal)
}

#[derive(Deserialize)]
struct PaperSize {
    width: f64,
    height: f64,
}

#[derive(Deserialize)]
struct Margins {
    top: f64,
    bottom: f64,
    left: f64,
    right: f64,
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Uso: {} <base64_json_config>", args[0]);
        std::process::exit(1);
    }

    // Decodificar configuración base64
    let config_json = STANDARD
        .decode(&args[1])
        .context("Fallo al decodificar la configuración base64")?;
    let config: Config = serde_json::from_slice(&config_json)
        .context("Fallo al deserializar la configuración JSON")?;

    // Convertir el contenido HTML en una data URL base64
    // Nota: si el HTML es muy grande, esto puede ser lento o costoso en memoria
    let data_url = format!(
        "data:text/html;base64,{}",
        STANDARD.encode(&config.html_content)
    );

    // Configurar Chrome
    let options = LaunchOptionsBuilder::default()
        .path(Some(config.chrome_path.into()))
        .headless(true)
        .sandbox(false) // Se deja en false para que funcione en el server
        // Algunos flags recomendados
        .args(vec![
            std::ffi::OsStr::new("--no-sandbox"),
            std::ffi::OsStr::new("--disable-dev-shm-usage"),
        ])
        .build()
        .context("No se pudo construir las LaunchOptions")?;

    let browser = Browser::new(options).context("No se pudo lanzar Chrome")?;
    let tab = browser
        .new_tab()
        .context("No se pudo crear una nueva pestaña")?;

    // Navegar a la data URL
    tab.navigate_to(&data_url)
        .context("No se pudo navegar al data URL")?;
    tab.wait_until_navigated()
        .context("Falló la navegación para renderizar el contenido")?;

    // Configurar opciones del PDF
    // Si orientation = "landscape", intercambiamos ancho y alto
    let (paper_width, paper_height) = if config.orientation == "landscape" {
        (config.paper_size.height, config.paper_size.width)
    } else {
        (config.paper_size.width, config.paper_size.height)
    };

    let pdf_options = PrintToPdfOptions {
        landscape: Some(config.orientation == "landscape"),
        display_header_footer: Some(false),
        print_background: Some(true),
        scale: Some(1.0),
        paper_width: Some(paper_width),
        paper_height: Some(paper_height),
        margin_top: Some(config.margins.top),
        margin_bottom: Some(config.margins.bottom),
        margin_left: Some(config.margins.left),
        margin_right: Some(config.margins.right),
        page_ranges: Some("1-".to_string()),
        ignore_invalid_page_ranges: Some(false),
        header_template: None,
        footer_template: None,
        prefer_css_page_size: Some(false),
        transfer_mode: None,
    };

    // Generar PDF
    let pdf_data = tab
        .print_to_pdf(Some(pdf_options))
        .context("Falló la generación del PDF")?;

    // Imprimir el PDF en base64 a stdout
    println!("{}", STANDARD.encode(&pdf_data));

    Ok(())
}
