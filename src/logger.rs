//! logger.rs
//! Configuración del logger usando env_logger.

use std::fs::File;

use env_logger::{Builder, Target};

pub fn init_logger() {
    let stdout = File::create("/var/log/pdf_service.log").unwrap();
    let _stderr = File::create("/var/log/pdf_service.error.log").unwrap();

    Builder::from_env(env_logger::Env::default().default_filter_or("debug"))
        .format_timestamp_secs()
        .target(Target::Pipe(Box::new(stdout))) // logs normales a stdout
        .write_style(env_logger::WriteStyle::Never)
        .init();
}
