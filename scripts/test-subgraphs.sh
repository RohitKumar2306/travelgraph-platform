#!/usr/bin/env bash
# scripts/test-subgraphs.sh
#
# Curl-based smoke test for every subgraph. Waits up to ~2 minutes for
# /actuator/health on each service to come back UP, then runs one
# representative GraphQL query and asserts the response has no `errors` key.
#
# Run via:   make test-subgraphs   (or directly: ./scripts/test-subgraphs.sh)

set -euo pipefail

# ANSI colors only when stdout is a tty.
if [[ -t 1 ]]; then
  GREEN=$'\033[0;32m'; RED=$'\033[0;31m'; YELLOW=$'\033[0;33m'; RESET=$'\033[0m'
else
  GREEN=""; RED=""; YELLOW=""; RESET=""
fi

# Seeded ids that every subgraph knows about (match the V1 migrations).
USER_ID="80000000-0000-0000-0000-000000000001"
PROPERTY_ID="11111111-1111-1111-1111-000000000001"

wait_for_health() {
  local name=$1 port=$2
  for _ in $(seq 1 60); do
    if curl -fsS "http://localhost:${port}/actuator/health" >/dev/null 2>&1; then
      return 0
    fi
    sleep 2
  done
  echo "${RED}TIMEOUT waiting for ${name} on port ${port}${RESET}"
  return 1
}

# Run a smoke query whose JSON payload is supplied verbatim to avoid escaping pain.
run_query() {
  local name=$1 port=$2 payload=$3
  local body
  if ! body=$(curl -fsS "http://localhost:${port}/graphql" \
        -H 'content-type: application/json' \
        -H "x-user-id: ${USER_ID}" \
        -d "$payload"); then
    echo "${RED}FAIL ${name}${RESET}  (HTTP error)"
    return 1
  fi
  if grep -q '"errors"' <<<"$body"; then
    echo "${RED}FAIL ${name}${RESET}"
    echo "      payload:  $payload"
    echo "      response: $body"
    return 1
  fi
  echo "${GREEN}OK   ${name}${RESET}  -> $body"
}

declare -a SERVICES=(
  "property:8081"
  "pricing:8082"
  "booking:8083"
  "user:8084"
  "review:8085"
)

echo "${YELLOW}Waiting for subgraphs to come up...${RESET}"
for svc in "${SERVICES[@]}"; do
  IFS=":" read -r name port <<<"$svc"
  wait_for_health "$name" "$port"
done

echo
echo "${YELLOW}Running smoke queries...${RESET}"

errors=0

run_query "property" 8081 \
  '{"query":"{ searchProperties(city: \"Austin\") { id name rating } }"}' \
  || errors=$((errors+1))

run_query "pricing"  8082 \
  "{\"query\":\"{ price(propertyId: \\\"${PROPERTY_ID}\\\") { totalAmount currency nights } }\"}" \
  || errors=$((errors+1))

run_query "booking"  8083 \
  "{\"query\":\"{ bookings(userId: \\\"${USER_ID}\\\") { id status } }\"}" \
  || errors=$((errors+1))

run_query "user"     8084 \
  '{"query":"{ me { id name loyaltyStatus } }"}' \
  || errors=$((errors+1))

run_query "review"   8085 \
  "{\"query\":\"{ reviewSummary(propertyId: \\\"${PROPERTY_ID}\\\") { count averageRating } }\"}" \
  || errors=$((errors+1))

echo
if [[ "$errors" -eq 0 ]]; then
  echo "${GREEN}All 5 subgraphs returned successful queries.${RESET}"
else
  echo "${RED}${errors} subgraph(s) failed the smoke test.${RESET}"
fi

exit "$errors"
