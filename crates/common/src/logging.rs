use tracing::*;

pub fn init() {
    let filt = tracing_subscriber::EnvFilter::from_default_env();
    tracing_subscriber::fmt()
        .compact()
        .with_env_filter(filt)
        .init();
    info!("logging started");
}
