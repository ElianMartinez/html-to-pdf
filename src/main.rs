use anyhow::Result;
use wkhtmltopdf::{Orientation, PageSize, PdfApplication, Size};

#[tokio::main]
async fn main() -> Result<()> {
    let pdf_app = PdfApplication::new().expect("Failed to init PDF application");
    let mut pdfout = pdf_app
        .builder()
        .orientation(Orientation::Landscape)
        .margin(Size::Millimeters(10))
        .title("Factura")
        .page_size(PageSize::A4)
        .build_from_path(
            "/Users/elianezequielmartinezhernandez/Documents/projects/html-to-pdf/src/temp.html",
        )
        .expect("Failed to build PDF");

    pdfout
        .save("factura.pdf")
        .expect("Failed to save 'factura.pdf'");
    println!("PDF generado exitosamente como 'factura.pdf'");

    Ok(())
}
