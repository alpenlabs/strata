# `express-chaintsn`

Contains core implementation of the rollup chain state transition.

This is meant to avoid deps on std that aren't available in risc0 or sp1 envs,
either directly or through crates.

## Features

* `fullstd` - enabled when we are building for running in a normal client with tracing
