use tracing::*;

pub fn init() {
    tracing_subscriber::fmt().compact().init();
    info!("logging started");
}
