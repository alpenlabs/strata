export RUST_LOG=trace,hyper=warn,soketto=warn,jsonrpsee-server=warn,mio=warn,strata_btcio::rpc::client=warn
export LOG_LEVEL=debug
export NO_COLOR=1
export PATH=$PATH:$(realpath ../target/release)
export RUST_BACKTRACE=1
