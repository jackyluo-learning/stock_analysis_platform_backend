use tracing_subscriber::{
    fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter,
};
use tracing_appender::non_blocking::WorkerGuard;

/// Initializes a layered tracing system.
/// Returns a WorkerGuard that must be kept alive for the duration of the program 
/// to ensure logs are flushed correctly.
pub fn init_tracing() -> WorkerGuard {
    // 1. Layer for Console (Standard output, human-readable)
    let console_layer = fmt::layer()
        .with_target(false)
        .with_thread_ids(true)
        .with_line_number(true)
        .with_file(true);

    // 2. Layer for File (Daily rolling, JSON format for machine parsing)
    // Logs will be stored in the 'logs' directory
    let file_appender = tracing_appender::rolling::daily("logs", "app.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    
    let file_layer = fmt::layer()
        .with_writer(non_blocking)
        .json() // Structured logging
        .with_target(true);

    // 3. Combine everything into a Registry
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(console_layer)
        .with(file_layer)
        .init();

    guard
}
