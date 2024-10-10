# Strata Functional Tests

Tests will be added here when we have more functionality to test.

## Prerequisites

### `bitcoind`

Most tests depend upon `bitcoind` being available. The tests here execute
this binary and then, perform various tests.

```bash
# for MacOS
brew install bitcoin
```

```bash
# for Linux (x86_64)
curl -fsSLO --proto "=https" --tlsv1.2 https://bitcoin.org/bin/bitcoin-core-27.0/bitcoin-27.0-x86_64-linux-gnu.tar.gz
tar xzf bitcoin-27.0-x86_64-linux-gnu.tar.gz
sudo install -m 0755 -t /usr/local/bin bitcoin-27.0/bin/*
```

```bash
# check installed version
bitcoind --version
```

### Poetry

We use Poetry for managing the test dependencies.

```bash
# install via apt
apt install python3-poetry
# or install poetry via pip3
pip3 install poetry
# or install poetry via pipx
pipx install poetry
# or install poetry via homebrew
brew install poetry
# check version
poetry --version
```

Make sure you have installed Python 3.10 or higher.

## Running tests

```bash
./run_test.sh
```

## Running prover tasks

```bash
PROVER_TEST=1 ./run_test.sh fn_prover_client.py
```

The test harness script will be extended with more functionality as we need it.
