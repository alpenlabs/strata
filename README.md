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
cargo run --bin alpen-express-reth  -- --datadir .data/reth --http -vvvv

# Block witness generation mode
cargo run --bin alpen-express-reth  -- --datadir .data/reth --http --enable-witness-gen -vvvv
```

Start CL Client/Sequencer

```sh
cargo run --bin alpen-express-sequencer -- --config config.toml
```

## Contribution Guidelines

### Development Tools

Please install the following tools in your development environment to make sure that
you can run the basic CI checks in your local environment:

- `taplo`

  This is a tool that is used to lint and format `TOML` files. You can install it with:
  
  ```bash
  brew install taplo
  ```
  
  You can learn more [here](https://taplo.tamasfe.dev/cli/installation/binary.html).

- `codespell`

  This is a tool that is used to check for common misspellings in code. You can install it with:
  
  ```bash
  pip install codespell # or `pip3 install codespell`
  ```
  
  You can learn more [here](https://github.com/codespell-project/codespell).

- `nextest`

  This is a modern test runner for Rust. You can install it with:
  
  ```bash
  cargo install --locked nextest
  ```
  
  Learn more [here](https://nexte.st).

- Functional test runner

  For dependencies required to run functional tests, see instructions in its [`README.md`](./functional-tests/README.md).

### Before Creating a PR

Before you create a PR, make sure that all the required CI checks pass locally.
For your convenience, a `Makefile` recipe has been created which you can run via:

```bash
make pr # `make` should already be installed in most systems
```
