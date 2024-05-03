use tracing::*;

use alpen_vertex_common::logging;

fn main() {
    logging::init();

    // TODO init RPC server and whatnot

    info!("exiting");
}
