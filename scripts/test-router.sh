#!/usr/bin/env bash
# Curl-based end-to-end smoke test for the Phase 2 router.
#
# Assumes `make router-up` has already been run (or `docker compose
# --profile subgraphs --profile router up -d --build`). Exits non-zero on the
# first failure so it can be wired into CI as-is.

set -euo pipefail

ROUTER_HOST="${ROUTER_HOST:-localhost}"
ROUTER_PORT="${ROUTER_PORT:-8080}"
BASE="http://${ROUTER_HOST}:${ROUTER_PORT}"

bold() { printf '\033[1m%s\033[0m\n' "$*"; }
green() { printf '\033[32m%s\033[0m\n' "$*"; }
red()   { printf '\033[31m%s\033[0m\n' "$*" >&2; }

wait_for_health() {
  bold "==> waiting for router /health on ${BASE}"
  for _ in $(seq 1 60); do
    if curl -fsS "${BASE}/health" >/dev/null 2>&1; then
      green "    /health OK"
      return 0
    fi
    sleep 1
  done
  red "router /health never came up"
  exit 1
}

run_query() {
  local label="$1"
  local body="$2"
  bold "==> ${label}"
  local resp
  resp="$(curl -fsS -X POST "${BASE}/graphql" \
    -H 'content-type: application/json' \
    --data-raw "${body}")"
  echo "${resp}"
  if grep -q '"errors"' <<<"${resp}" && ! grep -q '"data"' <<<"${resp}"; then
    red "    ${label}: GraphQL errors with no data"
    exit 1
  fi
  green "    ${label} OK"
}

wait_for_health

run_query "single-subgraph (property only)" \
  '{"query":"{ searchProperties(city: \"Austin\") { id name rating } }"}'

run_query "single-subgraph (review only)" \
  '{"query":"{ reviewSummary(propertyId: \"00000000-0000-0000-0000-000000000001\") { count averageRating } }"}'

run_query "two-subgraph parallel (property + review summary)" \
  '{"query":"{ searchProperties(city: \"Austin\") { id name } reviewSummary(propertyId: \"00000000-0000-0000-0000-000000000001\") { count } }"}'

run_query "validation: undeclared variable returns errors[]" \
  '{"query":"query Q { searchProperties(city: $city) { id } }"}'

bold "==> all router smoke tests passed"
