# syntax = docker/dockerfile:1.2
FROM lukemathwalker/cargo-chef:latest-rust-1-alpine as chef
WORKDIR /app
ARG BUILD_DEPS
RUN apk add --no-cache ${BUILD_DEPS}
# Prepare the dinner
FROM chef as planner
COPY . .
ARG RUSTFLAGS=-Ctarget-feature=-crt-static
RUN cargo chef prepare --recipe-path recipe.json
# Cook the dinner
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
ARG RUSTFLAGS=-Ctarget-feature=-crt-static
RUN --mount=type=cache,target=$CARGO_HOME/git \
    --mount=type=cache,target=$CARGO_HOME/registry \
    cargo chef cook --release --recipe-path recipe.json
COPY . .
# Serve the dinner to cargo
RUN --mount=type=cache,target=$CARGO_HOME/git \
    --mount=type=cache,target=$CARGO_HOME/registry \
    cargo -V; cargo build --release --bin twitch-discord-moderation && mv /app/target/release/twitch-discord-moderation /app/twitch-discord-moderation
FROM alpine:3 as runtime
WORKDIR /app
ARG RUN_DEPS
RUN apk add --no-cache \
        ${RUN_DEPS}
COPY --from=builder /app/twitch-discord-moderation /app/twitch-discord-moderation
ENTRYPOINT "/app/twitch-discord-moderation"