# Alpen Express Rollup

Alpen's prototype rollup, codenamed Express. This is pre-alpha software and
nothing is even close to being usable yet.

## Repository structure

### Binaries

Currently we only have a the sequencer that operates the rollup and provides
an RPC interface that end users can call to interact with the rollup ledger.
We will have standalone clients for end users to run their own rollup full nodes
at a future point, on the roadmap to decentralizing sequencing.

### Library crates

These exist in `crates/`.

* `btcio` - L1 reader/writer infra
* `common` - utils for services
* `consensus-logic` - consensus state machine impl
* `db` - Database abstractions
* `eectl` - EL exec control infra
* `evmexec` - utils relating to EVM execution via REVM
* `primitives` - common types used throughout project, mostly re-exports
* `rpc/api` - Alpen rollup RPC defs
* `state` - type defs relating to rollup data structures
* `storage` - intermediate storage IO abstraction layer
* `status` - contains utilities for handling node status 
* `util/` - independent utility libraries
  * `mmr` - "merkle mountain range" util
* `vtxjmt` - extensions to JMT crate for our purposes

### How to run

Prerequisite: 
  * bitcoin regtest instance with json-rpc access
    * host:port for bitcoind rpc `BITCOIND_HOST`, 
    * auth for bitcoind rpc: `BITCOIND_USER`:`BITCOIND_PASSWORD`
  * 32 byte sequencer key saved to file `SEQUENCER_KEY_PATH`
  * 32 byte EL client jwt secret saved as **hex** to file `JWT_SECRET_PATH`

Create `config.toml` for rollup (Or use `example_config.toml` as template)

```toml
[bitcoind_rpc]
rpc_url = "{BITCOIND_HOST}"
rpc_user = "{BITCOIND_USER}"
rpc_password = "{BITCOIND_PASSWORD}"
network = "regtest"

[client]
rpc_port = 8432
datadir = ".data/rollup"
sequencer_key = "{SEQUENCER_KEY_PATH}"

[sync]
l1_follow_distance = 6
max_reorg_depth = 4
client_poll_dur_ms = 2000

[exec.reth]
rpc_url = "http://localhost:8551"
secret = "{JWT_SECRET_PATH}"
```

Ensure bitcoin has some blocks

in `sequencer/src/main.rs`, adjust rollup configs: 
  * `horizon_l1_height` 
  * `genesis_l1_height` 

ensure `horizon_l1_height` <= `genesis_l1_height` < bitcoin_block_height

Start EL Client:

```sh
# Normal mode
cargo run --bin strata-reth  -- --datadir .data/reth --http -vvvv

# Block witness generation mode
cargo run --bin strata-reth  -- --datadir .data/reth --http --enable-witness-gen -vvvv
```

Start CL Client/Sequencer

```sh
cargo run --bin strata-client -- --config config.toml
```

## License

This work is dual-licensed under MIT and Apache 2.0.
You can choose between one of them if you use this work.
