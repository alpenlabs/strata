use tracing::*;

use alpen_nero_common::logging;

fn main() {
    logging::init();

    info!("exiting");
}
