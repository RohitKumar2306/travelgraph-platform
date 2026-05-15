# Load Test Results

No real load-test measurements have been captured yet in this workspace.

## Environment

- Date: 2026-05-15
- Host: not measured
- Docker: unavailable in this run
- k6: unavailable in this run
- Router URL: `http://localhost:8080/graphql`
- Prometheus URL: `http://localhost:9090`

## Results

| Scenario | Throughput | p50 | p95 | p99 | Error rate | Cache hit ratio | Notes |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| normal-traffic | not measured | not measured | not measured | not measured | not measured | not measured | Docker daemon was unavailable. |
| high-throughput | not measured | not measured | not measured | not measured | not measured | not measured | Docker daemon was unavailable. |
| failure-scenario | not measured | not measured | not measured | not measured | not measured | not measured | Docker daemon was unavailable. |
| cache-comparison | not measured | not measured | not measured | not measured | not measured | not measured | Docker daemon was unavailable. |

## How to Capture Real Numbers

```sh
load-tests/run-all.sh
```

The runner starts the local docker-compose deployment, runs all four k6 scripts,
writes JSON summaries to `load-tests/results/`, captures Prometheus cache and
circuit-breaker snapshots after each run, and rewrites this file with the run
environment.

Grafana screenshots should be saved into `load-tests/results/` while each run is
active. Do not replace this table with estimates.

