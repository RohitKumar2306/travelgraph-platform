#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RESULTS_DIR="${RESULTS_DIR:-$ROOT/load-tests/results}"
ROUTER_URL="${ROUTER_URL:-http://localhost:8080/graphql}"
PROMETHEUS_URL="${PROMETHEUS_URL:-http://localhost:9090}"
RUN_COMPOSE="${RUN_COMPOSE:-true}"

mkdir -p "$RESULTS_DIR"

need() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    exit 1
  fi
}

need docker
need k6

if [ "$RUN_COMPOSE" = "true" ]; then
  docker compose --profile subgraphs --profile router --profile observability up -d --build
fi

wait_for_router() {
  for _ in $(seq 1 120); do
    if curl -fsS "${ROUTER_URL%/graphql}/health" >/dev/null 2>&1; then
      return 0
    fi
    sleep 2
  done
  echo "router did not become healthy at ${ROUTER_URL%/graphql}/health" >&2
  exit 1
}

snapshot_prometheus() {
  local name="$1"
  local file="$RESULTS_DIR/${name}-prometheus.json"
  {
    printf '{\n'
    printf '  "capturedAt": "%s",\n' "$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
    printf '  "cacheHits": '
    curl -fsS --get "$PROMETHEUS_URL/api/v1/query" --data-urlencode 'query=graphql_cache_hits_total' || printf 'null'
    printf ',\n  "cacheMisses": '
    curl -fsS --get "$PROMETHEUS_URL/api/v1/query" --data-urlencode 'query=graphql_cache_misses_total' || printf 'null'
    printf ',\n  "circuitBreakerOpen": '
    curl -fsS --get "$PROMETHEUS_URL/api/v1/query" --data-urlencode 'query=graphql_circuit_breaker_open' || printf 'null'
    printf '\n}\n'
  } > "$file"
}

run_k6() {
  local name="$1"
  local script="$2"
  echo "==> $name"
  ROUTER_URL="$ROUTER_URL" PROMETHEUS_URL="$PROMETHEUS_URL" RESULTS_DIR="$RESULTS_DIR" k6 run "$ROOT/load-tests/$script"
  snapshot_prometheus "$name"
}

write_results_md() {
  local results_md="$ROOT/load-tests/RESULTS.md"
  {
    printf '# Load Test Results\n\n'
    printf 'Measured at: `%s`\n\n' "$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
    printf '## Environment\n\n'
    printf '- Host: `%s`\n' "$(uname -a)"
    printf '- Docker: `%s`\n' "$(docker version --format '{{.Server.Version}}' 2>/dev/null || echo unavailable)"
    printf '- k6: `%s`\n' "$(k6 version 2>/dev/null | head -n 1 || echo unavailable)"
    printf '- Router URL: `%s`\n' "$ROUTER_URL"
    printf '- Prometheus URL: `%s`\n\n' "$PROMETHEUS_URL"
    printf '## Result Files\n\n'
    find "$RESULTS_DIR" -maxdepth 1 -type f \( -name '*.json' -o -name '*.png' \) | sort | sed "s#^$ROOT/#- #"
    printf '\n## Notes\n\n'
    printf 'Numbers above are emitted by k6 JSON summaries. Prometheus snapshots capture cache and circuit-breaker counters after each run.\n'
    printf 'Attach Grafana screenshots captured during each run next to the JSON files in `load-tests/results/`.\n'
  } > "$results_md"
}

wait_for_router
run_k6 normal-traffic normal-traffic.js
run_k6 high-throughput high-throughput.js
run_k6 failure-scenario failure-scenario.js
run_k6 cache-comparison cache-comparison.js
write_results_md
