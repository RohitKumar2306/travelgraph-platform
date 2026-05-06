#!/usr/bin/env bash
#
# Phase 3.2 acceptance test:
#   * runs the composer against all 5 subgraphs and verifies that
#     `schema-registry/supergraph/supergraph.graphql` is produced;
#   * confirms the composed SDL contains entries for every subgraph in
#     `enum join__Graph` and federation directives on the Property entity.
#
# Assumes the subgraph stack is already up (`make up`).

set -euo pipefail

bold() { printf "\033[1;34m%s\033[0m\n" "$1"; }
ok()   { printf "  \033[32m%s\033[0m\n" "$1"; }
fail() { printf "  \033[31m%s\033[0m\n" "$1"; exit 1; }

OUTPUT="schema-registry/supergraph/supergraph.graphql"

bold "==> running schema composer"
docker compose --profile subgraphs --profile composer build composer >/dev/null
docker compose --profile subgraphs --profile composer run --rm composer
if [[ ! -s "$OUTPUT" ]]; then
  fail "composer ran but did not produce $OUTPUT"
fi
ok "supergraph written to $OUTPUT ($(wc -c < "$OUTPUT") bytes)"

bold "==> verifying enum join__Graph contains all 5 subgraphs"
for sg in property pricing booking user review; do
  if grep -qE "@join__graph\\(\\s*name:\\s*\"${sg}\"" "$OUTPUT"; then
    ok "  found subgraph: $sg"
  else
    fail "missing subgraph in supergraph: $sg"
  fi
done

bold "==> verifying Property entity has owner + 2 extenders"
if grep -qE "type Property" "$OUTPUT" && grep -qE "@join__type\\(graph: PROPERTY" "$OUTPUT"; then
  ok "Property declared with owner subgraph"
else
  fail "Property type or owner directive missing"
fi
for ext in PRICING REVIEW; do
  if grep -qE "@join__type\\(graph: $ext.*extension: true" "$OUTPUT"; then
    ok "Property extended by $ext"
  else
    fail "Property is missing extension by $ext"
  fi
done

bold "==> all composer acceptance checks passed"
