export RUST_LOG=trace,hyper=warn,soketto=warn,jsonrpsee-server=warn,mio=warn
export NO_COLOR=1
export PATH=$PATH:$(realpath ../target/release)
export RUST_BACKTRACE=1
export HACK_SKIP_UPDATE_CONTINUITY_CHECK=1
