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
