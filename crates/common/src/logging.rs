use opentelemetry_otlp::WithExportConfig;
use tracing::*;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

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

pub fn init(config: LoggerConfig) {
    let filt = tracing_subscriber::EnvFilter::from_default_env();

    let mut loggers: Vec<Box<dyn tracing::Subscriber>> = Vec::new();

    // Stdout logging.
    let stdout_sub = tracing_subscriber::fmt()
        .compact()
        .with_env_filter(filt)
        .finish();
    //loggers.push(Box::new(stdout_sub));

    // OpenTelemetry output.
    if let Some(otel_url) = &config.otel_url {
        let exporter = opentelemetry_otlp::new_exporter()
            .tonic()
            .with_endpoint(otel_url);

        let tracer = opentelemetry_otlp::new_pipeline()
            .tracing()
            .with_exporter(exporter)
            .install_batch(opentelemetry_sdk::runtime::TokioCurrentThread)
            .expect("init: opentelemetry");

        // TODO

        let otel_sub = tracing_opentelemetry::OpenTelemetryLayer::new(tracer);

        //loggers.append(Box::new(stdout_sub));
    }

    info!(whoami = %config.whoami, "logging started");
}
