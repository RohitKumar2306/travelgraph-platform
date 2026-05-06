#!/usr/bin/env node
//
// Apollo Federation composer.
//
// Usage:
//   node compose.mjs <subgraphs.json> <output.graphql>
//
// `<subgraphs.json>` is a list of `{name, url, sdl}` objects produced by the
// Kotlin CLI after fetching `_service { sdl }` from each subgraph. The script:
//
//   1. Parses each SDL via graphql-js,
//   2. Calls `composeServices` from @apollo/composition,
//   3. On success: writes the supergraph SDL to <output.graphql>, then prints
//      `{ "ok": true, "output": "<output.graphql>" }` to stdout.
//   4. On failure: prints `{ "ok": false, "errors": [...] }` to stdout and
//      exits non-zero. Each error has `{ message, code, nodes }` where `code`
//      is the Apollo composition error code (e.g. FIELD_TYPE_MISMATCH).

import { readFileSync, writeFileSync } from 'node:fs';
import { parse } from 'graphql';
import { composeServices } from '@apollo/composition';

const [, , inputPath, outputPath] = process.argv;
if (!inputPath || !outputPath) {
  console.error('usage: compose.mjs <subgraphs.json> <output.graphql>');
  process.exit(64);
}

const raw = readFileSync(inputPath, 'utf8');
const subgraphs = JSON.parse(raw);

const services = subgraphs.map((s) => {
  if (!s || typeof s.name !== 'string' || typeof s.url !== 'string' || typeof s.sdl !== 'string') {
    throw new Error(`invalid subgraph entry: ${JSON.stringify(s)}`);
  }
  return { name: s.name, url: s.url, typeDefs: parse(s.sdl) };
});

const result = composeServices(services);

if (result.errors && result.errors.length > 0) {
  const payload = {
    ok: false,
    errors: result.errors.map((e) => ({
      message: e.message,
      code: e.extensions?.code ?? null,
      nodes: (e.nodes || []).map((n) => ({
        kind: n.kind,
        loc: n.loc ? { start: n.loc.start, end: n.loc.end } : null,
      })),
    })),
  };
  process.stdout.write(`${JSON.stringify(payload, null, 2)}\n`);
  process.exit(2);
}

writeFileSync(outputPath, result.supergraphSdl);
process.stdout.write(`${JSON.stringify({ ok: true, output: outputPath }, null, 2)}\n`);
