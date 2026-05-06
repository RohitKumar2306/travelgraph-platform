# TravelGraph Platform - top-level Makefile
#
# All commands assume Docker Desktop / a Docker daemon is running. The
# `subgraphs` compose profile gates the five subgraph services so we can
# bring the platform up with one command.

.DEFAULT_GOAL := help
SHELL := /bin/bash

COMPOSE := docker compose
PROFILE := --profile subgraphs

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
