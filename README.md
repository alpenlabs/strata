# Alpen Vertex Rollup

Alpen's prototype rollup, codenamed Vertex.  This is pre-alpha software and
nothing is even close to being usable yet.

## Repository structure

### Binaries

Currently we only have a the sequencer that operates the rollup and provides
an RPC interface that end users can call to interact with the rollup ledger.
We will have standalone clients for end users to run their own rollup full nodes
at a future point, on the roadmap to decentralizing sequencing.

### Library crates

These exist in `crates/`.

* `common` - utils for services
* `consensus-logic` - consensus state machine impl
* `db` - Database abstractions
* `evmexec` - utils relating to EVM execution via REVM
* `primitives` - common types used throughout project, mostly re-exports
* `rpc/api` - Alpen rollup RPC defs
* `state` - type defs relating to rollup data strctures
