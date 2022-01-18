# syntax = docker/dockerfile:1.2
FROM --platform=$BUILDPLATFORM rust:1.58-alpine3.15 as planner
WORKDIR /app
RUN apk add --no-cache \
        musl-dev
FROM rust:1.58-alpine3.14 as builder
WORKDIR /app
ARG BUILD_DEPS
RUN apk add --no-cache ${BUILD_DEPS}
COPY . .
ARG RUSTFLAGS=-Ctarget-feature=-crt-static
RUN --mount=type=cache,target=$CARGO_HOME/git \
    --mount=type=cache,target=$CARGO_HOME/registry \
    --mount=type=cache,sharing=private,target=/app/target \
    cargo -V; cargo build --release --bin twitch-discord-moderation && mv /app/target/release/twitch-discord-moderation /app/twitch-discord-moderation
FROM alpine:3.15 as runtime
WORKDIR /app
ARG RUN_DEPS
RUN apk add --no-cache \
        ${RUN_DEPS}
COPY --from=builder /app/twitch-discord-moderation /app/twitch-discord-moderation
ENTRYPOINT "/app/twitch-discord-moderation"