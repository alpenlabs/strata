use opentelemetry::trace::TracerProvider;
use opentelemetry_otlp::WithExportConfig;
use tracing::*;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Layer};

pub struct LoggerConfig {
    whoami: String,
    otel_url: Option<String>,
}

impl LoggerConfig {
    pub fn new(whoami: String) -> Self {
        Self {
            whoami,
            otel_url: None,
        }
    }
}

/// Initializes the logging subsystem with the provided config.
pub fn init(config: LoggerConfig) {
    let filt = tracing_subscriber::EnvFilter::from_default_env();

    // TODO switch to using subscribers everywhere instead of layers
    //let mut loggers: Vec<Box<dyn tracing::Subscriber + 'static>> = Vec::new();

    // Stdout logging.
    let stdout_sub = tracing_subscriber::fmt::layer().compact().with_filter(filt);

    // OpenTelemetry output.
    if let Some(otel_url) = &config.otel_url {
        let exporter = opentelemetry_otlp::new_exporter()
            .tonic()
            .with_endpoint(otel_url);

        let tp = opentelemetry_otlp::new_pipeline()
            .tracing()
            .with_exporter(exporter)
            .install_batch(opentelemetry_sdk::runtime::TokioCurrentThread)
            .expect("init: opentelemetry");

        let tt = tp.tracer("strata-log");

        let otel_sub = tracing_opentelemetry::layer().with_tracer(tt);

        tracing_subscriber::registry()
            .with(stdout_sub)
            .with(otel_sub)
            .init();
    } else {
        tracing_subscriber::registry().with(stdout_sub).init();
    }

    info!(whoami = %config.whoami, "logging started");
}

/// Shuts down the logging subsystem, flushing files as needed and tearing down
/// resources.
pub fn finalize() {
    info!("shutting down logging");
    // TODO
}
