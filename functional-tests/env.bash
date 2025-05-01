export RUST_LOG=trace,hyper=warn,soketto=warn,jsonrpsee-server=warn,mio=warn,bitcoind-async-client::client=warn,trie=warn
export NO_COLOR=1
export PATH=$PATH:$(realpath ../target/release)
export RUST_BACKTRACE=1
export LOG_LEVEL=info
