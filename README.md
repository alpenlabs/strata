# Strata

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache-blue.svg)](https://opensource.org/licenses/apache-2-0)
[![codecov](https://codecov.io/gh/alpenlabs/strata/branch/main/graph/badge.svg?token=Q3ZYY44GN7)](https://codecov.io/gh/alpenlabs/strata)
[![ci](https://github.com/alpenlabs/strata/actions/workflows/lint.yml/badge.svg?event=push)](https://github.com/alpenlabs/strata/actions)
[![docs](https://img.shields.io/badge/docs-strata-orange)](https://docs.stratabtc.org)

<p align="center">
  <img src="https://docs.stratabtc.org/images/Strata_Small.png" alt="Strata Logo" width="21%">
</p>

[**Strata**](https://stratabtc.org) gives developers the freedom to program nearly
any locking conditions for BTC imaginable,
limited only by the Strata block size and gas limits.
This enables developers to create new kinds of applications for BTC
with features such as:

- **New signature types**, "provide a valid `P-256` signature to authorize a transfer"

- **Vaults**, "transfers must wait `N` days after being initiated to be effectuated,
  and can be cancelled in the mean time"

- **Subscriptions**, "address `0x123...9a` can withdraw up to `v` BTC 
  per month from this account"

- **Strong privacy**, "transaction details are end-to-end encrypted
  and verified using a zero-knowledge proof"

- **Economically-secured zero-confirmation payments**,
  "if a double-spend from this sender is reported,
  the reporter gets to claim the sender's full wallet balance"

- **Financial transactions**,
  "if enough BTC is locked as collateral to maintain up
  to `X%` loan-to-value ratio,
  then up to N of this other asset can be borrowed"

... and many more possibilities.

Technically speaking,
**Strata is a work-in-progress EVM-compatible validity rollup on bitcoin**.
Let's break down what this means:

- **EVM-compatible**: The Strata block producer runs a client that is based on
  [Reth](https://github.com/paradigmxyz/reth),
  an Ethereum execution client.
  So far, no changes have been made that affect compatibility with the EVM spec.
  If you can deploy a smart contract to Ethereum,
  you can deploy it to Strata with no changes.

- **Validity rollup**: Every Strata state transition is proven to
  be valid using cryptographic validity proofs,
  which clients can use for fast, low-cost verification.

- **On bitcoin**: Strata uses bitcoin for consensus and data availability.
  When a Strata block gets confirmed on bitcoin,
  the only way to reorganize this block is to reorganize
  the bitcoin block that the Strata block was confirmed in.

To learn more, check our [documentation](https://docs.stratabtc.org).

> [!IMPORTANT]
> During the devnet phase,
> Strata will be running on a private bitcoin signet,
> and will use signet blocks to store state commitments rather than
> the complete Strata state data,
> making Strata function more like a commit chain than a rollup.
> Support for full onchain data availability and for running Strata
> on bitcoin mainnet are planned for future releases.

## Repository structure

This repository is composed of:

- `bin/`: binary crates for various clients and CLIs
- `crates/`: library crates, provides types and functionalities
- `docker/`: supporting files for our dockerized applications
- `functional-tests/`: end-to-end tests for various scenarios
- `provers/`: libraries and binaries related to zero-knowledge proofs
- `tests/`: integration tests

## Contributing

Contributions are generally welcome.
If you intend to make larger changes please discuss them in an issue
before opening a PR to avoid duplicate work and architectural mismatches.

For more information please see [`CONTRIBUTING.md`](/CONTRIBUTING.md).

## License

This work is dual-licensed under MIT and Apache 2.0.
You can choose between one of them if you use this work.
