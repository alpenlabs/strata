export RUST_LOG=trace,hyper=warn,soketto=warn,jsonrpsee-server=warn,mio=warn
export NO_COLOR=1
export PATH=$PATH:$(realpath ../target/release)
export RUST_BACKTRACE=1
# export RUST_BACKTRACE=0
# export RUST_LOG=info,hyper=warn,soketto=warn,jsonrpsee-server=warn,mio=warn
