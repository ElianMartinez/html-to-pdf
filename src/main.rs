use anyhow::Result;
use headless_chrome::types::PrintToPdfOptions; // Importación corregida
use headless_chrome::{Browser, LaunchOptionsBuilder};
use std::fs;

#[tokio::main]
async fn main() -> Result<()> {
    // Creamos el HTML de ejemplo

    // Configuramos las opciones de Chrome
    let options = LaunchOptionsBuilder::default()
        .headless(true)
        .window_size(Some((1920, 1080)))
        .build()?;

    // Iniciamos el navegador
    let browser = Browser::new(options)?;

    // Creamos una nueva pestaña
    let tab = browser.new_tab()?;

    // Cargamos el archivo HTML
    tab.navigate_to(&format!(
        "file://{}",
        std::env::current_dir()?
            .join("src/temp.html")
            .to_str()
            .unwrap()
    ))?;

    // Esperamos a que la página se cargue completamente
    tab.wait_until_navigated()?;

    // Configuramos las opciones del PDF
    let pdf_options = PrintToPdfOptions {
        landscape: Some(false),
        display_header_footer: Some(false),
        print_background: Some(true),
        scale: Some(1.0),
        paper_width: Some(11.69), // 297 mm / 25.4
        paper_height: Some(8.27), // 210 mm / 25.4
        margin_top: Some(0.4),
        margin_bottom: Some(0.4),
        margin_left: Some(0.4),
        margin_right: Some(0.4),
        page_ranges: Some("1-".to_string()),
        ignore_invalid_page_ranges: Some(false),
        header_template: Some("".to_string()),
        footer_template: Some("".to_string()),
        prefer_css_page_size: Some(false),
        transfer_mode: None,
    };

    // Generamos el PDF
    let pdf_data = tab.print_to_pdf(Some(pdf_options))?;

    // Guardamos el PDF
    fs::write("factura.pdf", pdf_data)?;

    println!("PDF generado exitosamente como 'factura.pdf'");

    Ok(())
}
