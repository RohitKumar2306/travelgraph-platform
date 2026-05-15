#!/usr/bin/env node
import { readFileSync, writeFileSync } from "node:fs";
import { parse } from "graphql";
import { composeServices } from "@apollo/composition";

const [, , inputPath, outputPath] = process.argv;
if (!inputPath || !outputPath) {
  console.error("usage: compose.mjs <subgraphs.json> <output.graphql>");
  process.exit(64);
}

const subgraphs = JSON.parse(readFileSync(inputPath, "utf8"));
const services = subgraphs.map((s) => ({
  name: s.name,
  url: s.url,
  typeDefs: parse(s.sdl)
}));

const result = composeServices(services);
if (result.errors?.length) {
  process.stdout.write(JSON.stringify({
    ok: false,
    errors: result.errors.map((e) => ({
      message: e.message,
      code: e.extensions?.code ?? null
    }))
  }, null, 2));
  process.exit(2);
}

writeFileSync(outputPath, result.supergraphSdl);
process.stdout.write(JSON.stringify({ ok: true, output: outputPath }, null, 2));
