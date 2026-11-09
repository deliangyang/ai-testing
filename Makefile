# Convenience targets for local development.
# Quality gate of record: scripts/check.sh (Windows: scripts/check.ps1).
# See AGENTS.md §11 for the canonical command list.

.PHONY: help check fmt clippy test guards build clean \
	cli mock-sut validate run-serial run-parallel run-async \
	web-install web-dev web-test web-e2e web-build web-lint \
	schemas

SHELL := /bin/bash
.DEFAULT_GOAL := help

CARGO ?= cargo
NPM ?= npm
CHECK ?= ./scripts/check.sh
CLI := $(CARGO) run -q -p cli --
MYTOOL_BIN := target/debug/mytool
DAG ?= examples/serial.yaml
ENV ?= local
DB ?= .aitest/aitest.db
REPORT ?= out/report.json

help: ## Show available targets
	@awk 'BEGIN {FS = ":.*##"; printf "Usage: make <target>\n\n"} \
		/^[a-zA-Z0-9_-]+:.*?##/ { printf "  %-16s %s\n", $$1, $$2 }' $(MAKEFILE_LIST)

# --- quality gate -----------------------------------------------------------

check: ## Full quality gate (fmt + clippy + test + guards + web)
	$(CHECK)

fmt: ## Format check only
	$(CHECK) fmt

clippy: ## Clippy + web lint
	$(CHECK) clippy

test: ## Workspace + web tests
	$(CHECK) test

guards: ## Architecture red-line guards only
	$(CHECK) guards

# --- rust -------------------------------------------------------------------

build: ## Debug build of the whole workspace
	$(CARGO) build --workspace

$(MYTOOL_BIN):
	$(CARGO) build -p cli

cli: $(MYTOOL_BIN) ## Build the mytool binary
	@echo "built $(MYTOOL_BIN)"

mock-sut: ## Start the mock system under test
	$(CARGO) run -p mock-sut

schemas: ## Regenerate JSON schemas from Rust types
	UPDATE_SCHEMAS=1 $(CARGO) test -p model --test schema_sync

# --- example DAGs -----------------------------------------------------------

validate: ## Validate DAG file (DAG=examples/serial.yaml)
	$(CLI) dag validate $(DAG)

run-serial: ## Run serial example (--no-llm)
	@mkdir -p "$(dir $(REPORT))"
	$(CLI) --db $(DB) run --dag examples/serial.yaml --env $(ENV) --no-llm --report-json $(REPORT)

run-parallel: ## Run parallel example (--no-llm)
	@mkdir -p "$(dir $(REPORT))"
	$(CLI) --db $(DB) run --dag examples/parallel.yaml --env $(ENV) --no-llm --report-json $(REPORT)

run-async: ## Run async-poll example (--no-llm)
	@mkdir -p "$(dir $(REPORT))"
	$(CLI) --db $(DB) run --dag examples/async-poll.yaml --env $(ENV) --no-llm --report-json $(REPORT)

# --- web --------------------------------------------------------------------

web-install: ## npm install in web/
	cd web && $(NPM) install

web-dev: ## Vite dev server (fixture data source)
	cd web && $(NPM) run dev

web-test: ## Frontend unit tests
	cd web && $(NPM) test

web-e2e: ## Playwright E2E
	cd web && $(NPM) run test:e2e

web-build: ## Frontend production build
	cd web && $(NPM) run build

web-lint: ## Frontend eslint + prettier check
	cd web && $(NPM) run lint && $(NPM) run format:check

# --- housekeeping -----------------------------------------------------------

clean: ## Remove Rust/web build artifacts and local reports
	$(CARGO) clean
	rm -rf web/dist out
