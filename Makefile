# TravelGraph Platform - top-level Makefile
#
# All commands assume Docker Desktop / a Docker daemon is running. The
# `subgraphs` compose profile gates the five subgraph services so we can
# bring the platform up with one command.

.DEFAULT_GOAL := help
SHELL := /bin/bash

COMPOSE     := docker compose
PROFILE     := --profile subgraphs
ALL_PROFILE := --profile subgraphs --profile router

# ---------------------------------------------------------------------------
# Top-level lifecycle
# ---------------------------------------------------------------------------

.PHONY: help
help: ## Show this help.
	@grep -E '^[a-zA-Z_.-]+:.*?## .*$$' $(MAKEFILE_LIST) | \
	  awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-20s\033[0m %s\n", $$1, $$2}'

.PHONY: up
up: ## Build & start postgres + all subgraph services in the background.
	$(COMPOSE) $(PROFILE) up -d --build

.PHONY: down
down: ## Stop & remove all containers (data volumes are kept).
	$(COMPOSE) $(PROFILE) down

.PHONY: nuke
nuke: ## Stop everything and drop the postgres volume. Re-runs all Flyway seeds on next `up`.
	$(COMPOSE) $(PROFILE) down -v

.PHONY: logs
logs: ## Tail logs from every service.
	$(COMPOSE) $(PROFILE) logs -f --tail=200

.PHONY: ps
ps: ## Show running services.
	$(COMPOSE) $(PROFILE) ps

# ---------------------------------------------------------------------------
# Data
# ---------------------------------------------------------------------------

.PHONY: seed
seed: ## (No-op) Flyway migrations + seed inserts run automatically on container start.
	@echo "Seed data is loaded by Flyway when each subgraph container starts."
	@echo "If you need a fresh seed, run: make nuke && make up"

# ---------------------------------------------------------------------------
# Smoke / verification
# ---------------------------------------------------------------------------

.PHONY: test-subgraphs
test-subgraphs: ## Run the curl-based smoke test against every subgraph's /graphql.
	@./scripts/test-subgraphs.sh

.PHONY: test
test: ## Run unit tests for every subgraph (uses the gradle docker image, no local JDK needed).
	@for svc in property-service pricing-service booking-service user-service review-service; do \
	  echo "==> $$svc"; \
	  docker run --rm -v "$$PWD/services/$$svc":/work -w /work \
	    gradle:8.10.2-jdk21 gradle --no-daemon test || exit 1; \
	done

# ---------------------------------------------------------------------------
# Router (Phase 2)
# ---------------------------------------------------------------------------

.PHONY: router-up
router-up: ## Build & start the Rust router on top of the subgraph stack.
	$(COMPOSE) $(ALL_PROFILE) up -d --build

.PHONY: router-down
router-down: ## Stop everything (subgraphs + router).
	$(COMPOSE) $(ALL_PROFILE) down

.PHONY: router-logs
router-logs: ## Tail the router's stdout (JSON-formatted tracing events).
	$(COMPOSE) logs -f --tail=200 router

.PHONY: router-build
router-build: ## Build the router image only.
	$(COMPOSE) $(ALL_PROFILE) build router

.PHONY: router-test
router-test: ## Run the router's unit tests inside the official rust toolchain.
	@docker run --rm -v "$$PWD/router":/work -w /work \
	  rust:1.86 cargo test --manifest-path Cargo.toml

.PHONY: router-image-size
router-image-size: ## Show the runtime image size; Phase 2.1 budget is <50MB.
	@docker image inspect travelgraph/router:dev --format 'travelgraph/router:dev = {{.Size}} bytes ({{div .Size 1048576}} MiB)'

.PHONY: test-router
test-router: ## Curl-based end-to-end smoke test against the router (/health + /graphql).
	@./scripts/test-router.sh
