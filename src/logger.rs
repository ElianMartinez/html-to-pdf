//! logger.rs
//! Configuración del logger usando env_logger.

use env_logger;

pub fn init_logger() {
    // Podrías leer la variable RUST_LOG del entorno (por ejemplo)
    // para configurar el nivel de logs. Si no está, definimos un default.
    let log_env = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_env))
        .format_timestamp_secs()
        .init();
}
