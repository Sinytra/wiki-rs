ARG RUST_IMAGE=rust:1.95.0-slim-bookworm
ARG RUNTIME_IMAGE=debian:bookworm-slim

FROM ${RUST_IMAGE} AS chef
WORKDIR /app
ENV CARGO_TERM_COLOR=always \
    CARGO_NET_RETRY=10 \
    RUSTUP_MAX_RETRIES=10 \
    CARGO_INCREMENTAL=0

# Install dependencies
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        pkg-config libssl-dev zlib1g-dev cmake build-essential ca-certificates git curl \
    && rm -rf /var/lib/apt/lists/*

# Install Sentry CLI
RUN curl -sL https://sentry.io/get-cli/ | sh

# Install Rust toolchain
RUN rustup toolchain install stable --profile minimal --no-self-update \
    && rustup default stable
RUN cargo install cargo-chef --locked --version ^0.1

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

COPY . .
RUN cargo build --release --bin wiki-service \
    && objcopy --only-keep-debug --compress-debug-sections=zlib target/release/wiki-service target/release/wiki-service.d \
    && objcopy --strip-debug --strip-unneeded target/release/wiki-service \
    && objcopy --add-gnu-debuglink=target/release/wiki-service.d target/release/wiki-service \
    && cp target/release/wiki-service /usr/local/bin/wiki-service

RUN cargo build --release -p wiki-migration --bin wiki-migration \
    && objcopy --strip-debug --strip-unneeded target/release/wiki-migration \
    && cp target/release/wiki-migration /usr/local/bin/wiki-migration

FROM ${RUNTIME_IMAGE} AS runtime-base

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates libssl3 zlib1g \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system --gid 1000 wiki \
    && useradd  --system --uid 1000 --gid wiki --home /app --shell /sbin/nologin wiki

WORKDIR /app
USER wiki

FROM runtime-base AS migrations

COPY --from=builder /usr/local/bin/wiki-migration /usr/local/bin/wiki-migration

ENTRYPOINT ["/usr/local/bin/wiki-migration"]

FROM runtime-base AS runtime

COPY --from=builder /usr/local/bin/wiki-service /usr/local/bin/wiki-service
COPY --from=builder /app/builtin /app/builtin
ENV WIKI_STORAGE__BUILTIN_DATA_PATH=/app/builtin

EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/wiki-service"]
