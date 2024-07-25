# Heavily inspired by Reth: https://github.com/paradigmxyz/reth/blob/d599393771f9d7d137ea4abf271e1bd118184c73/Makefile
.DEFAULT_GOAL := help

GIT_TAG ?= $(shell git describe --tags --abbrev=0)

BUILD_PATH = "target"

# Cargo profile for builds. Default is for local builds, CI uses an override.
PROFILE ?= release

# Extra flags for Cargo
CARGO_INSTALL_EXTRA_FLAGS ?=

# List of features to use for building
FEATURES ?=

# The docker image name
DOCKER_IMAGE_NAME ?=

##@ Help

.PHONY: help
help: ## Display this help.
	@awk 'BEGIN {FS = ":.*##"; printf "Usage:\n  make \033[36m<target>\033[0m\n"} /^[a-zA-Z_0-9-]+:.*?##/ { printf "  \033[36m%-15s\033[0m %s\n", $$1, $$2 } /^##@/ { printf "\n\033[1m%s\033[0m\n", substr($$0, 5) } ' $(MAKEFILE_LIST)

##@ Build

.PHONY: build
build: ## Build the workspace into the `target` directory.
	cargo build --workspace --bin "alpen-vertex-sequencer" --features "$(FEATURES)" --profile "$(PROFILE)"

##@ Test

UNIT_TEST_ARGS := --locked --workspace -E 'kind(lib)' -E 'kind(bin)' -E 'kind(proc-macro)'
COV_FILE := lcov.info

.PHONY: test-unit
test-unit: ## Run unit tests.
	cargo install cargo-nextest --locked
	cargo nextest run $(UNIT_TEST_ARGS)

.PHONY: cov-unit
cov-unit: ## Run unit tests with coverage.
	rm -f $(COV_FILE)
	cargo llvm-cov nextest --lcov --output-path $(COV_FILE) $(UNIT_TEST_ARGS)

.PHONY: cov-report-html
cov-report-html: cov-unit ## Generate a HTML coverage report and open it in the browser.
	cargo llvm-cov --open

.PHONY: mutants-test
mutants-test: ## Runs `nextest` under `cargo-mutants`. Caution: This can take *really* long to run.
	cargo mutants --workspace -j2

.PHONY: sec
sec: ## Check for security advisories on any dependencies.
	cargo audit #  HACK: not denying warnings as we depend on `yaml-rust` via `format-serde-error` which is unmaintained

##@ Functional Tests
.PHONY: ensure-poetry
ensure-poetry:
	@if ! command -v poetry &> /dev/null; then \
		echo "poetry not found. Please install it by running the command `cargo install taplo-cli --locked` or refer to the following link for more information: https://taplo.tamasfe.dev/cli/installation/binary.html" \
		exit 1; \
    fi

.PHONY: activate
activate: ensure-poetry ## Activate poetry environment for integration tests.
	cd functional-tests && poetry install --no-root

.PHONY: clean-dd
clean-dd:
	rm -rf test/_dd 2>/dev/null

.PHONY: test-functional
test-functional: ensure-poetry clean-dd ## Runs functional tests.
	cd functional-tests && ./run_test.sh

##@ Code Quality

.PHONY: fmt-check-ws
fmt-check-ws: ## Check formatting issues but do not fix automatically.
	cargo fmt --check

.PHONY: fmt-ws
fmt-ws: ## Format source code in the workspace.
	cargo fmt --all

.PHONY: ensure-taplo
ensure-taplo:
	@if ! command -v taplo &> /dev/null; then \
		echo "taplo not found. Please install it by running the command `cargo install taplo-cli --locked` or refer to the following link for more information: https://taplo.tamasfe.dev/cli/installation/binary.html" \
		exit 1; \
    fi

.PHONY: fmt-check-toml
fmt-check-toml: ensure-taplo ## Runs `taplo` to check that TOML files are properly formatted
	taplo fmt --check

.PHONY: fmt-toml
fmt-toml: ensure-taplo ## Runs `taplo` to format TOML files
	taplo fmt

ensure-ruff:
	@if ! command -v ruff &> /dev/null; then \
		echo "ruff not found. Please install it by running the command `pip install ruff` or refer to the following link for more information: https://docs.astral.sh/ruff/installation/" \
		exit 1; \
    fi

.PHONY: fmt-check-func-tests
fmt-check-func-tests: ensure-ruff ## Check formatting of python files inside `test` directory.
	cd functional-tests && ruff format --check

.PHONY: fmt-func-tests
fmt-func-tests: ensure-ruff ## Apply formatting of python files inside `test` directory.
	cd functional-tests && ruff format

.PHONY: lint-check-ws
lint-check-ws: ## Checks for lint issues in the workspace.
	cargo clippy \
	--workspace \
	--bin "alpen-vertex-sequencer" \
	--lib \
	--examples \
	--tests \
	--benches \
	-- -D warnings

.PHONY: lint-fix-ws
lint-fix-ws: ## Lints the workspace and applies fixes where possible.
	cargo clippy \
	--workspace \
	--bin "alpen-vertex-sequencer" \
	--lib \
	--examples \
	--tests \
	--benches \
	--fix \
	-- -D warnings

ensure-codespell:
	@if ! command -v codespell &> /dev/null; then \
		echo "codespell not found. Please install it by running the command `pip install codespell` or refer to the following link for more information: https://github.com/codespell-project/codespell" \
		exit 1; \
    fi

.PHONY: lint-codepsell
lint-check-codespell: ensure-codespell ## Runs `codespell` to check for spelling errors.
	codespell

.PHONY: lint-fix-codepsell
lint-fix-codespell: ensure-codespell ## Runs `codespell` to fix spelling errors if possible.
	codespell -w

.PHONY: lint-toml
lint-check-toml: ensure-taplo ## Lints TOML files
	taplo lint

.PHONY: lint-check-func-tests
lint-check-func-tests: ensure-ruff ## Lints python files inside the `test` directory.
	cd functional-tests && ruff check

.PHONY: lint-fix-func-tests
lint-fix-func-tests: ensure-ruff ## Runs lint fixes for python files inside `test` directory.
	cd functional-tests && ruff check --fix

.PHONY: lint
lint: ## Runs all lints and checks for issues without trying to fix them.
	make lint-check-ws && \
	make lint-check-codespell && \
	make lint-check-toml && \
	make fmt-check-toml && \
	make fmt-check-ws

.PHONY: lint-fix
lint-fix: ## Runs all lints and applies fixes where possible.
	make lint-fix-ws && \
	make lint-fix-codespell && \
	make fmt-toml && \
	make fmt-ws

.PHONY: rustdocs
rustdocs: ## Runs `cargo docs` to generate the Rust documents in the `target/doc` directory.
	RUSTDOCFLAGS="\
	--show-type-layout \
	--enable-index-page -Z unstable-options \
	-D warnings" \
	cargo doc \
	--workspace \
	--document-private-items

.PHONY: test-doc
test-doc: ## Runs doctests on the workspace.
	cargo test --doc --workspace

.PHONY: test
test: ## Runs all tests in the workspace including unit and docs tests.
	make test-unit && \
	make test-doc

.PHONY: pr
pr: ## Runs lints (without fixing) and unit tests (run this before creating a PR).
	make lint && \
	make sec && \
	make test-unit
