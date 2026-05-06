# syntax=docker/dockerfile:1.7
#
# travelgraph-composer image. Bundles:
#   * the Kotlin CLI (built with Gradle)
#   * Node.js + the @apollo/composition Node helper (pre-installed)
#
# Run inside the docker-compose network so the composer can reach each
# subgraph by service name:
#
#   docker compose run --rm composer --output /supergraph/supergraph.graphql

# ---- builder: Gradle + JDK 21 ---------------------------------------------
FROM gradle:8.10.2-jdk21 AS jvm-builder
WORKDIR /work
COPY settings.gradle.kts build.gradle.kts ./
COPY src ./src
RUN gradle --no-daemon installDist

# ---- helper: pre-install the Node compose dependencies --------------------
FROM node:20-alpine AS node-builder
WORKDIR /helper
COPY src/main/resources/package.json ./
COPY src/main/resources/compose.mjs ./
RUN npm install --omit=dev --no-audit --no-fund

# ---- runtime: JDK 21 + Node 20 --------------------------------------------
FROM eclipse-temurin:21-jre-alpine

RUN apk add --no-cache nodejs npm bash

WORKDIR /app
COPY --from=jvm-builder /work/build/install/travelgraph-composer ./travelgraph-composer
COPY --from=node-builder /helper /app/node-helper
COPY composer.toml /app/composer.toml

ENV TRAVELGRAPH_COMPOSER_HELPER=/app/node-helper

ENTRYPOINT ["/app/travelgraph-composer/bin/travelgraph-composer"]
CMD ["--config", "/app/composer.toml", "--output", "/supergraph/supergraph.graphql"]
