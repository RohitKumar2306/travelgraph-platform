# syntax=docker/dockerfile:1.7
#
# Multi-stage build for the TravelGraph router.
#
# Stage 1 (builder)  - rust:1.81 with the full toolchain. Runs cargo build.
# Stage 2 (runtime)  - distroless cc-debian12 (~22MB base) plus our binary.
#
# The runtime image is GLIBC-based because reqwest's resolver uses platform
# DNS via getaddrinfo. With rustls-tls and a stripped release binary the
# total image weighs in well under the Phase 2.1 budget of 50MB.

# ---- builder ---------------------------------------------------------------
# `apollo-compiler` 1.31 transitively pulls icu_* crates that require rustc
# >=1.86, so we pin a known-good toolchain image rather than tracking
# `rust:latest`.
FROM rust:1.86 AS builder

WORKDIR /workspace

# Cache deps separately from sources for fast rebuilds.
COPY Cargo.toml Cargo.lock* ./
COPY crates/travelgraph-router/Cargo.toml crates/travelgraph-router/Cargo.toml

# Pre-build a stub so the dependency graph compiles independently of source
# changes. This caches the heavy `apollo-compiler` / `reqwest` builds.
RUN mkdir -p crates/travelgraph-router/src \
 && echo "fn main() {}" > crates/travelgraph-router/src/main.rs \
 && cargo build --release --locked || cargo build --release \
 && rm -rf crates/travelgraph-router/src

COPY crates ./crates
COPY config ./config

# Real build. `--locked` is best-effort - we'll regenerate Cargo.lock if it
# isn't already checked in.
RUN cargo build --release --locked || cargo build --release

# ---- runtime ---------------------------------------------------------------
FROM gcr.io/distroless/cc-debian12:nonroot

WORKDIR /app

COPY --from=builder /workspace/target/release/travelgraph-router /app/travelgraph-router
COPY --from=builder /workspace/config /app/config

EXPOSE 8080

USER nonroot
ENV ROUTER_CONFIG=/app/config/router.toml

ENTRYPOINT ["/app/travelgraph-router"]
