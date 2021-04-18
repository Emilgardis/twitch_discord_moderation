# syntax = docker/dockerfile:experimental
# TODO specify cross compilation instead
FROM rust:1.51-slim as builder
WORKDIR /app
RUN \
    apt-get update && \
	apt-get install -yqq --no-install-recommends \
    libssl-dev libgit2-dev git pkg-config && rm -rf /var/lib/apt/lists/*
COPY . .
RUN --mount=type=cache,target=$CARGO_HOME/registry --mount=type=cache,target=/app/target cargo build --release --bin twitch-discord-moderation; mv /app/target/release/twitch-discord-moderation /app/twitch-discord-moderation
FROM debian:buster-slim as runtime
WORKDIR /app
RUN \
    apt-get update && \
	apt-get install -yqq --no-install-recommends \
    openssl ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/twitch-discord-moderation /app/twitch-discord-moderation
ENTRYPOINT "/app/twitch-discord-moderation"