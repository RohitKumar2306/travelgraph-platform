#!/usr/bin/env bash
#
# Phase 3 end-to-end acceptance test.
#
# 1. Each subgraph answers `query { _service { sdl } }`.
# 2. Pricing and Review SDLs declare the Property entity with @key.
# 3. The router (already up) accepts a federated query that requires
#    Property + Pricing + Review and stitches the response.
#
# Run after: `make federation-up`.

set -euo pipefail

bold() { printf "\033[1;34m%s\033[0m\n" "$1"; }
ok()   { printf "  \033[32m%s\033[0m\n" "$1"; }
fail() { printf "  \033[31m%s\033[0m\n" "$1"; exit 1; }

declare -A SUBGRAPHS=(
  [property]=8081
  [pricing]=8082
  [booking]=8083
  [user]=8084
  [review]=8085
)

bold "==> Phase 3.1: every subgraph answers \`{ _service { sdl } }\`"
for sg in "${!SUBGRAPHS[@]}"; do
  port="${SUBGRAPHS[$sg]}"
  body=$(curl -s -X POST "http://localhost:${port}/graphql" \
    -H 'content-type: application/json' \
    --data '{"query":"{ _service { sdl } }"}')
  if printf '%s' "$body" | grep -q '"sdl"'; then
    ok "${sg} (port ${port}) returned SDL"
  else
    fail "${sg} did not return SDL: ${body}"
  fi
done

bold "==> Phase 3.1: Pricing + Review schemas show extend type Property @key"
for sg in pricing review; do
  port="${SUBGRAPHS[$sg]}"
  body=$(curl -s -X POST "http://localhost:${port}/graphql" \
    -H 'content-type: application/json' \
    --data '{"query":"{ _service { sdl } }"}')
  sdl=$(printf '%s' "$body" | python3 -c 'import json,sys; d=json.load(sys.stdin); print(d["data"]["_service"]["sdl"])')
  if printf '%s' "$sdl" | grep -qE 'type Property\s*@key\(fields: ?"id"\)\s*@extends|@extends.*type Property\s*@key|extend\s+type\s+Property\s*@key'; then
    ok "${sg} declares Property @key + @extends"
  else
    fail "${sg} SDL does not show Property @key + @extends:\n${sdl}"
  fi
done

bold "==> Phase 3.1: _entities returns the right type for a Property representation"
PROP_ID=$(curl -s -X POST 'http://localhost:8081/graphql' \
  -H 'content-type: application/json' \
  --data '{"query":"{ searchProperties(city: \"Austin\") { id } }"}' \
  | python3 -c 'import json,sys; d=json.load(sys.stdin); print(d["data"]["searchProperties"][0]["id"])')
ok "picked Property id: ${PROP_ID}"

ents=$(curl -s -X POST 'http://localhost:8082/graphql' \
  -H 'content-type: application/json' \
  --data "{\"query\":\"query(\$reps: [_Any!]!) { _entities(representations: \$reps) { ... on Property { price { totalAmount } } } }\",\"variables\":{\"reps\":[{\"__typename\":\"Property\",\"id\":\"${PROP_ID}\"}]}}")
if printf '%s' "$ents" | grep -q '"totalAmount"'; then
  ok "Pricing _entities returned a price quote"
else
  fail "Pricing _entities did not return a price quote: ${ents}"
fi

bold "==> Phase 3.4: router stitches Property + Pricing + Review with batched _entities"
resp=$(curl -s -X POST 'http://localhost:8080/graphql' \
  -H 'content-type: application/json' \
  --data '{"query":"{ searchProperties(city: \"Austin\") { name price { totalAmount } reviews { rating } } }"}')
if printf '%s' "$resp" | grep -q '"totalAmount"' && printf '%s' "$resp" | grep -q '"rating"'; then
  ok "router returned merged Property + price + reviews"
else
  fail "router stitch failed: ${resp}"
fi

bold "==> all federation acceptance checks passed"
