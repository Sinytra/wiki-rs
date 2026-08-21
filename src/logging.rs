use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{Builder, Rotation};
use tracing_subscriber::{EnvFilter, Layer, fmt, layer::SubscriberExt, util::SubscriberInitExt};

pub struct LogGuards {
    _file: WorkerGuard,
}

pub struct LoggingConfig {
    pub dir: String,
    pub file_prefix: String,
    pub default_filter: String,
    pub max_files: usize,
    pub storage_path: String,
}

pub fn init(cfg: &LoggingConfig) -> LogGuards {
    let console_layer = fmt::layer()
        .compact()
        .with_ansi(true)
        .with_target(true)
        .with_thread_names(true);

    let file_appender = Builder::new()
        .rotation(Rotation::DAILY)
        .filename_prefix(&cfg.file_prefix)
        .filename_suffix("log")
        .max_log_files(cfg.max_files)
        .build(&cfg.dir)
        .expect("failed to initialize rolling file appender");
    let (file_writer, file_guard) = tracing_appender::non_blocking(file_appender);
    let file_layer = fmt::layer()
        .with_writer(file_writer)
        .with_ansi(false)
        .with_target(true)
        .with_thread_names(true)
        .with_line_number(true);

    let make_filter = || {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&cfg.default_filter))
    };

    tracing_subscriber::registry()
        .with(console_layer.with_filter(make_filter()))
        .with(file_layer.with_filter(make_filter()))
        .with(
            sentry::integrations::tracing::layer()
                .enable_span_attributes()
                .with_filter(make_filter()),
        )
        .with(wiki_storage::deployment::log::layer(
            cfg.storage_path.clone().into(),
        ))
        .init();

    LogGuards { _file: file_guard }
}
